use std::{fs, net::{Ipv4Addr, SocketAddr}, path::PathBuf, time::Duration};

use anyhow::{Error, Result, anyhow};
use clap::{command, ArgAction, Parser};
use mc_scanner::{range, scanner::{Naive, Scan}};
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
    #[clap(flatten)]
    target: Target,
    #[clap(long, default_value = "./exclude.conf", help = "exclude file path. automatically generated if not populated")]
    exclude_file: PathBuf,
    //#[clap(value_parser = humantime::parse_duration, default_value = "1000ms")]
    //interval: Duration,
    #[clap(short, long, help = "use async implementation (masscan-style)")]
    asynchronous: bool,
    #[clap(short, long, help = "use naive implementation", required_if_eq("asynchronous", "true"))]
    source_port: Option<u16>,
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
    let targets_filtered = range::apply_exclude(target_ranges, exclude_list);

    let (tx, mut rx) = mpsc::channel(100);

    if args.asynchronous {
        unimplemented!(); 
    } else {
        Naive::new(25565, 32767).scan(targets_filtered, tx);
    }

    while let Some((ip, res)) = rx.recv().await {
        println!("ip: {}, {:?}", ip, res);
    }


    return Ok(());
}
