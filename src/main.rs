use std::{fs, net::{Ipv4Addr, SocketAddr}, path::PathBuf, time::Duration};

use anyhow::{Error, Result, anyhow};
use clap::{command, Parser};
use ipnet::{Ipv4AddrRange, Ipv4Net};
use mc_scanner::{range::{self}, slp::{SlpError, StatusResponse}};
use mc_scanner::slp;
use tokio::sync::mpsc;

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

    let exclude_list = range::parse_masscan(&exclude_str)?;
    let target_ranges = range::parse_masscan(&args.target.to_str()?)?;
    println!("{:?}", target_ranges);
    let targets_filtered = range::apply_exclude(target_ranges, exclude_list);
    println!("{:?}", targets_filtered);

    let threads: usize = 32768;
    let chunked_targets = range::chunk_ranges(targets_filtered, threads.try_into().unwrap());
    let total_targets = chunked_targets.len();

    let (tx, mut rx) = mpsc::channel(100);

    for target_ranges in &chunked_targets {
        let tx = tx.clone();
        let target_ranges = target_ranges.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            for (start, end) in target_ranges {
                let start = u32::from(start);
                let end = u32::from(end);
                for ip_u in start..=end {
                    let ip = Ipv4Addr::from(ip_u);
                    let sock_addr = SocketAddr::from((ip, 25565));
                    let res = slp!(sock_addr).await;

                    tx.send((ip, res)).await.expect("failed to tx");
                }
            }
        });
    }

    drop(tx);
    drop(chunked_targets);

    println!("scanning...");

    while let Some((ip, res)) = rx.recv().await {
        if let Ok(res) = res {
            println!("msg recieved for: {}, version: {:?}", ip, res.version);
        }
    }


    return Ok(());
}
