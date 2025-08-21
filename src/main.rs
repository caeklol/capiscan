use std::{fs, net::{Ipv4Addr, SocketAddr}, path::PathBuf, sync::{atomic::{AtomicUsize, Ordering}, Arc, RwLock}, time::{Duration, Instant}};

use anyhow::{Error, Result, anyhow};
use clap::{command, ArgAction, Parser};
use humantime::DurationError;
use mc_scanner::{range, scanner::{Async, Naive, Scan, ScanEvent}, slp::StatusResponse};
use tokio::sync::{mpsc, Mutex};

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

#[derive(Default)]
struct ProgramState {
    pub transmitted: usize,
    pub discovered: Vec<(Ipv4Addr, StatusResponse)>,
}


// not to be confused with the async TCP recieve thread, this thread recieves messages from the
// current scanner implementation and saves necessary values
fn receive_thread(state: Arc<Mutex<ProgramState>>, mut rx: mpsc::Receiver<ScanEvent>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            let mut state = state.lock().await;
            match event {
                mc_scanner::scanner::ScanEvent::Discovered(ip, res) => {
                    state.discovered.push((ip, res));
                },
                mc_scanner::scanner::ScanEvent::Transmitted(_) => {
                    state.transmitted += 1;
                },
            }
        }
    })
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

    let (tx, rx) = mpsc::channel(100);

    if args.asynchronous {
        Async::new(25565, args.source_port.unwrap()).scan(targets_filtered, tx);
    } else {
        Naive::new(25565, 32767).scan(targets_filtered, tx);
    }

    let state = Arc::new(Mutex::new(ProgramState::default()));
    let recieve_thread = receive_thread(state.clone(), rx);

    let mut last_transmitted = 0;
    let mut last_discovered = 0;

    while !recieve_thread.is_finished() {
        tokio::time::sleep(Duration::from_millis(1000)).await;
        let state = state.lock().await;
        let transmitted_count = state.transmitted - last_transmitted;
        let discovered_count = state.discovered.len() - last_discovered;
        println!("txd: {}, d: {}", transmitted_count / 1000, discovered_count);

        last_transmitted = state.transmitted;
        last_discovered = state.discovered.len();
        drop(state);
    }
       
    return Ok(());
}
