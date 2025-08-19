use std::{cmp::{max, min}, fs, iter, net::{Ipv4Addr, SocketAddr, SocketAddrV4}, path::PathBuf, str::FromStr, sync::{Arc, Mutex}, time::Duration};

use anyhow::{Error, Result, anyhow};
use clap::{arg, command, Parser};
use futures::{stream, StreamExt};
use ipnet::{Ipv4AddrRange, Ipv4Net};
use mc_scanner::slp;
use tokio::sync::Semaphore;

static DEFAULT_EXCLUDE_LIST: &str = include_str!("../data/exclude.conf");

#[derive(Debug, clap::Args)]
#[group(multiple = false)]
struct Target {
    #[clap(long)]
    target_file: Option<PathBuf>,
    #[clap(long, default_value = "0.0.0.0/0")]
    target: Option<String>,
}

impl Target {
    pub fn to_str(&self) -> Result<String, std::io::Error> {
        if let Some(file_path) = &self.target_file {
            Ok(fs::read_to_string(file_path)?)
        } else if let Some(target) = &self.target {
            Ok(target.clone())
        } else {
            unreachable!("clap should have never let this happen")
        }
    }
}

#[derive(Parser)]
#[command(name = "capiscan")]
#[command(author = "caek <me@caek.dev>")]
#[command(about = "basic server scanner", long_about = None)]
struct Args {
    #[clap(long, default_value = "./exclude.conf", help = "exclude file path. automatically generated if not populated")]
    exclude_file: PathBuf,
    #[clap(value_parser = humantime::parse_duration, default_value = "1000ms")]
    interval: Duration,
    #[clap(flatten)]
    target: Target,
}

fn parse_masscan_line(str: &str) -> Result<(Ipv4Addr, Ipv4Addr), Error> {
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

fn parse_masscan(str: &str) -> Result<Vec<(Ipv4Addr, Ipv4Addr)>, Error> {
    return str
        .lines()
        .into_iter()
        .filter(|s| !s.starts_with("#"))
        .filter(|s| !s.trim().is_empty())
        .map(|s| parse_masscan_line(s.trim()))
        .collect::<Result<Vec<(Ipv4Addr, Ipv4Addr)>, Error>>();
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args = Args::parse();

    let exclude_str = match fs::read_to_string(&args.exclude_file) {
        Ok(list) => list,
        Err(e) => {
            eprintln!("could not read exclude list: {}", e);
            println!("generating & using default exclude list");
            fs::write(args.exclude_file, DEFAULT_EXCLUDE_LIST)?;
            DEFAULT_EXCLUDE_LIST.to_owned()
        },
    };

    let mut exclude_list_unmerged = parse_masscan(&exclude_str)?
        .iter()
        .map(|t| (u32::from(t.0), u32::from(t.1)))
        .collect::<Vec<(u32, u32)>>();

    exclude_list_unmerged.sort_by_key(|f| f.0);

    let mut exclude_list = Vec::new();
    if let Some(first) = exclude_list_unmerged.get(0) {
        exclude_list.push(*first);
    }

    for i in 1..exclude_list_unmerged.len() {
        let curr = exclude_list_unmerged[i];
        let prev = exclude_list.last_mut().unwrap();

        if curr.0 <= prev.1 {
            prev.1 = max(prev.1, curr.1);
        } else {
            exclude_list.push(curr);
        }
    }
    
    let target_ranges = parse_masscan(&args.target.to_str()?)?
        .iter()
        .map(|t| (u32::from(t.0), u32::from(t.1)))
        .collect::<Vec<(u32, u32)>>()
        .chunk_by(|a, b| a.1 <= b.0)
        .map(|chunk| (chunk.first().unwrap().0, chunk.last().unwrap().1))
        .collect::<Vec<(u32, u32)>>()
        .iter()
        .flat_map(|range| {
            let start = range.0;
            let end = range.1;
            let mut current_start = start;
            let mut out = Vec::new();

            for (exclude_start, exclude_end) in &exclude_list {
                if *exclude_end <= current_start || *exclude_start >= end {
                    continue;
                }

                if *exclude_start > current_start {
                    out.push((current_start, *exclude_start - 1)); 
                }

                current_start = *exclude_end + 1;
            }

            if current_start < end {
                out.push((current_start, end)); 
            }

            return out;
        })
        .collect::<Vec<(u32, u32)>>();


    println!("scanning...");
    //println!("excluded_ranges: {:?}", exclude_list);
    println!("target_ranges: {:?}", target_ranges);

    return Ok(());
}
