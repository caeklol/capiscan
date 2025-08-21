use std::net::{Ipv4Addr, SocketAddr};

use tokio::sync::mpsc;

use crate::{range, slp::StatusResponse};
use crate::slp;

pub enum ScanEvent {
    Discovered(Ipv4Addr, StatusResponse),
    Transmitted(Ipv4Addr)
}

pub trait Scan {
    fn scan(&mut self, ranges: Vec<(Ipv4Addr, Ipv4Addr)>, tx: mpsc::Sender<ScanEvent>);
}

pub struct Naive {
    port: u16,
    threads: usize
}

impl Naive {
    pub fn new(port: u16, threads: usize) -> Self {
        Self {
            port,
            threads
        }
    }
}

impl Scan for Naive {
    fn scan(&mut self, ranges: Vec<(Ipv4Addr, Ipv4Addr)>, tx: mpsc::Sender<ScanEvent>) {
        let chunked_targets = range::chunk_ranges(ranges, self.threads.try_into().expect("cannot use more than u32::MAX threads!"));

        for target_ranges in &chunked_targets {
            let tx = tx.clone();
            let target_ranges = target_ranges.clone();
            let port = self.port.clone();

            tokio::spawn(async move {
                for (start, end) in target_ranges {
                    let start = u32::from(start);
                    let end = u32::from(end);
                    for ip_u in start..=end {
                        let ip = Ipv4Addr::from(ip_u);
                        let sock_addr = SocketAddr::from((ip, port));
                        let res = slp!(sock_addr).await;

                        
                        tx.send(ScanEvent::Transmitted(ip)).await.expect("failed to send message across channel");

                        if let Ok(res) = res {
                            tx.send(ScanEvent::Discovered(ip, res)).await.expect("failed to send message across channel");
                        }
                    }
                }
            });
        }
    }
}

pub struct Async {
    target_port: u16,
    source_port: u16
}

impl Async {
    pub fn new(source_port: u16, target_port: u16) -> Self {
        Self {
            target_port,
            source_port
        }
    }
}

impl Scan for Async {
    fn scan(&mut self, ranges: Vec<(Ipv4Addr, Ipv4Addr)>, tx: mpsc::Sender<ScanEvent>) {
        unimplemented!();
        for (start, end) in ranges {
            let start = u32::from(start);
            let end = u32::from(end);
            for ip in start..=end {

            }
        }
    }
}
