use std::{net::{Ipv4Addr, SocketAddr}, time::Duration};

use anyhow::Error;
use tokio::sync::mpsc;

use crate::{range, slp::{SlpError, StatusResponse}};
use crate::slp;

pub trait Scan {
    type Response;
    fn scan(&mut self, ranges: Vec<(Ipv4Addr, Ipv4Addr)>, tx: mpsc::Sender<Self::Response>);
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
    type Response = (Ipv4Addr, StatusResponse);

    fn scan(&mut self, ranges: Vec<(Ipv4Addr, Ipv4Addr)>, tx: mpsc::Sender<Self::Response>) {
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

                        if let Ok(res) = res {
                            tx.send((ip, res)).await.expect("failed to tx");
                        }
                    }
                }
            });
        }
    }
}
