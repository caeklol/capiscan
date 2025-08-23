use std::{net::{Ipv4Addr, SocketAddr}, path::PathBuf, sync::{atomic::{AtomicUsize, Ordering}, Arc, RwLock}, time::{Duration, Instant}};

use async_bincode::tokio::AsyncBincodeWriter;
use futures::SinkExt;
use anyhow::{Error, Result, anyhow};
use clap::{command, ArgAction, Parser};
use mc_scanner::{circ::CircularBuffer, range, scanner::{Async, Naive, Scan, ScanEvent}, slp::StatusResponse};
use serde::Serialize;
use tokio::{fs::{self, File}, io::AsyncWriteExt, sync::{mpsc, Mutex}};

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
    pub async fn to_str(&self) -> Result<String, std::io::Error> {
        if let Some(file_path) = &self.target_file {
            Ok(fs::read_to_string(file_path).await?)
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
    #[clap(long, default_value = "./capiscan.state", help = "program state path. progress and discovered servers is saved here atomically via tmp file")]
    state_file: PathBuf,
    #[clap(short, long, help = "save state directly to file")]
    no_atomic: bool,
    #[clap(long, default_value = "./exclude.conf", help = "exclude file path. automatically generated if not populated")]
    exclude_file: PathBuf,
    //#[clap(value_parser = humantime::parse_duration, default_value = "1000ms")]
    //interval: Duration,
    #[clap(short, long, help = "use async implementation (masscan-style)")]
    asynchronous: bool,
    #[clap(short, long, help = "use naive implementation", required_if_eq("asynchronous", "true"))]
    source_port: Option<u16>,
}

#[derive(Default, Serialize)]
struct ProgramState {
    pub transmitted: usize,
    pub discovered: Vec<(Ipv4Addr, StatusResponse)>,
}

// not to be confusedwith the async TCP recieve thread, this thread recieves messages from the
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

    let exclude_str = match fs::read_to_string(&args.exclude_file).await {
        Ok(list) => list,
        Err(e) => {
            eprintln!("could not read exclude list: {}", e);
            println!("generating & using default exclude list");
            fs::write(args.exclude_file, DEFAULT_EXCLUDE_LIST).await?;
            DEFAULT_EXCLUDE_LIST.to_owned()
        },
    };

    let exclude_list = range::parse_masscan(&exclude_str)?;
    let target_ranges = range::parse_masscan(&args.target.to_str().await?)?;
    let targets_filtered = range::apply_exclude(target_ranges, exclude_list);

    let state = Arc::new(Mutex::new(ProgramState::default()));
    let (tx, rx) = mpsc::channel(100);

    if args.asynchronous {
        Async::new(25565, args.source_port.unwrap()).scan(targets_filtered, tx);
    } else {
        Naive::new(25565, 32767).scan(targets_filtered, tx);
    }

    let recieve_thread = receive_thread(state.clone(), rx);

    let mut last_transmitted = 0;
    let mut last_discovered = 0;

    let mut last_kpps = CircularBuffer::new(7);
    let mut last_dps = CircularBuffer::new(7);

    let interval = Duration::from_millis(1000);

    while !recieve_thread.is_finished() {
        tokio::time::sleep(interval).await;
        let state = state.lock().await;
        let transmitted_count = state.transmitted - last_transmitted;
        let discovered_count = state.discovered.len() - last_discovered;

        let interval_ms = interval.as_millis() as f32;
        last_dps.push(discovered_count as f32 * (interval_ms / 1000.0));
        last_kpps.push(transmitted_count as f32 / (interval_ms * 10.0));

        let kpps = last_kpps.values().iter().fold(0.0, |acc, x| acc + *x) / last_dps.len() as f32;
        let dps = last_dps.values().iter().fold(0.0, |acc, x| acc + *x) / last_dps.len() as f32;

        println!("kpps: {}, dps: {} (discovered {} total)", kpps, dps, state.discovered.len());

        last_transmitted = state.transmitted;
        last_discovered = state.discovered.len();

        if args.no_atomic {
            let state_file = File::create(&args.state_file).await?;
            let mut writer = AsyncBincodeWriter::from(state_file).for_async();
            writer.send(&*state).await?;
        } else {
            let tmp_path = args.state_file.with_extension("swp");
            let state_file = File::create(&tmp_path).await?;
            let mut writer = AsyncBincodeWriter::from(state_file).for_async();

            if let Err(e) = writer.send(&*state).await {
                eprintln!("save to tmp file failed!");
                eprintln!("note: your `{}.swp` may not have the latest data", args.state_file.to_string_lossy());
                panic!("Error: {:#?}", e);
            }

            drop(state);

            if let Err(e) = tokio::fs::rename(&tmp_path, &args.state_file).await {
                eprintln!("rename from .swp to state_file failed!");
                eprintln!("note: your `{}.swp` file may have the latest data", args.state_file.to_string_lossy());
                panic!("Error: {:#?}", e);
            }
        }
    }
       
    return Ok(());
}
