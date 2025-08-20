use std::{cmp::{max, min}, fs, iter, net::{Ipv4Addr, SocketAddr, SocketAddrV4}, path::PathBuf, str::FromStr, sync::{Arc, Mutex}, time::Duration};

use anyhow::{Error, Result, anyhow};
use clap::{arg, command, Parser};
use futures::{stream, FutureExt, Stream, StreamExt};
use ipnet::{Ipv4AddrRange, Ipv4Net};
use mc_scanner::{parse::{self}, slp::{SlpError, StatusResponse}};
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

    let exclude_list = parse::parse_masscan(&exclude_str)?;
    let target_ranges = parse::parse_masscan(&args.target.to_str()?)?;
    let targets_filtered = parse::apply_exclude(target_ranges, exclude_list)
        .iter()
        .map(|t| (u32::from(t.0), u32::from(t.1)))
        .collect::<Vec<(u32, u32)>>();

    let chunked_targets = parse::chunk_ranges(targets_filtered, 20);
    
    // slp!(SocketAddr::from((ip, 25565))).await;

    println!("scanning...");

    return Ok(());
}
