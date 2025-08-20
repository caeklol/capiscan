use std::{cmp, str::FromStr};
use std::net::Ipv4Addr;

use anyhow::{Error, anyhow};
use ipnet::Ipv4Net;

pub fn parse_masscan_line(str: &str) -> Result<(Ipv4Addr, Ipv4Addr), Error> {
    if let Ok(cidr) = Ipv4Net::from_str(str) {
        return Ok((cidr.network(), cidr.broadcast()));
    }

    if let Ok(ip) = Ipv4Addr::from_str(str) {
        return Ok((ip, ip));
    }

    if let Some((start, end)) = str.split_once("-") {
        if start.trim().is_empty() || end.trim().is_empty() {
            return Err(anyhow!("start / end of range is empty: `{}`", str));
        }

        if let Ok(start) = Ipv4Addr::from_str(start) {
            if let Ok(end) = Ipv4Addr::from_str(end) {
                return Ok((start, end));
            } else {
                return Err(anyhow!("end of range not valid IP: `{}`", str));
            }
        } else {
            return Err(anyhow!("start of range not valid IP: `{}`", str));
        }
    }

    return Err(anyhow!("failed parsing masscan format on line: `{}`", str));
}

pub fn parse_masscan(str: &str) -> Result<Vec<(Ipv4Addr, Ipv4Addr)>, Error> {
    return str
        .lines()
        .into_iter()
        .filter(|s| !s.starts_with("#"))
        .filter(|s| !s.trim().is_empty())
        .map(|s| parse_masscan_line(s.trim()))
        .collect::<Result<Vec<(Ipv4Addr, Ipv4Addr)>, Error>>();
}

fn merge_intervals(intervals: Vec<(u32, u32)>) -> Vec<(u32, u32)> {
    let mut intervals = intervals;
    intervals.sort_by_key(|f| f.0);

    let mut out = Vec::new();
    if let Some(first) = intervals.get(0) {
        out.push(*first);
    }

    for i in 1..intervals.len() {
        let curr = intervals[i];
        let prev = out.last_mut().unwrap();

        if curr.0 <= prev.1 {
            prev.1 = cmp::max(prev.1, curr.1);
        } else {
            out.push(curr);
        }
    }

    out
}

pub fn apply_exclude(
    target: Vec<(Ipv4Addr, Ipv4Addr)>,
    exclude: Vec<(Ipv4Addr, Ipv4Addr)>
) -> Vec<(Ipv4Addr, Ipv4Addr)> {
    let target = target.iter()
        .map(|t| (u32::from(t.0), u32::from(t.1)))
        .collect::<Vec<(u32, u32)>>();

    let exclude = exclude.iter()
        .map(|t| (u32::from(t.0), u32::from(t.1)))
        .collect::<Vec<(u32, u32)>>();

    let exclude = merge_intervals(exclude);
    let target = merge_intervals(target);

    let ranges_applied = target.iter()
        .map(|t| (u32::from(t.0), u32::from(t.1)))
        .collect::<Vec<(u32, u32)>>()
        .iter()
        .flat_map(|range| {
            let start = range.0;
            let end = range.1;
            let mut current_start = start;
            let mut out = Vec::new();

            for (exclude_start, exclude_end) in &exclude {
                if *exclude_end <= current_start || *exclude_start >= end {
                    continue;
                }

                if *exclude_start > current_start {
                    out.push((current_start, *exclude_start - 1)); 
                }

                let exclude_end = *exclude_end;

                current_start = exclude_end.checked_add(1).unwrap_or(exclude_end);
            }

            if current_start < end {
                out.push((current_start, end)); 
            }

            return out;
        })
        .collect::<Vec<(u32, u32)>>();

    merge_intervals(ranges_applied)
        .into_iter()
        .map(|t| (Ipv4Addr::from(t.0), Ipv4Addr::from(t.1)))
        .collect::<Vec<(Ipv4Addr, Ipv4Addr)>>()
}

/// `chunks` is the amount of output chunks
///
/// ```
/// use mc_scanner::parse::chunk_ranges;
///
/// let ranges = vec![(0, 4), (6, 10), (20, 21)];
/// let chunked_ranges = chunk_ranges(ranges, 2);
///
/// assert_eq!(chunked_ranges, vec![vec![(0, 4), (6, 6)], vec![(7, 10), (20, 21)]])
/// assert_eq!(chunked_ranges.len(), 2);
/// ```
pub fn chunk_ranges(ranges: Vec<(u32, u32)>, chunks: u32) -> Vec<Vec<(u32, u32)>> {
    let total_span = ranges
        .iter()
        .fold(0u32, |acc, r| acc + (r.1-r.0) + 1);

    let span_per_chunk = total_span / chunks;

    let mut ranges = ranges.into_iter().peekable();
    let mut out = vec![vec![]];
    let mut acc = 0;

    while let Some(r) = ranges.peek_mut() {
        let span = (r.1 - r.0) + 1;
        let new_size = acc + span;

        if new_size == span_per_chunk {
            let r = ranges.next().unwrap();
            out.last_mut().unwrap().push(r);
            out.push(vec![]);
            acc = 0;
        } else if new_size > span_per_chunk {
            let excess = new_size - span_per_chunk;
            let new_r = (r.0, r.1 - excess);
            let new_span = (new_r.1 - new_r.0) + 1;

            r.0 += new_span;

            out.last_mut().unwrap().push(new_r);
            out.push(vec![]);
            acc = 0;
        } else if new_size < span_per_chunk {
            let r = ranges.next().unwrap();
            out.last_mut().unwrap().push(r);
            acc += span;
        }
    }

    if out.len() > chunks.try_into().unwrap() {
        out.pop();
    }

    out
}
