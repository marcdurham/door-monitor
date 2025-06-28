use clap::Parser;

mod config;
mod door;
mod audio;
mod utils;
mod sms;
mod monitor;

use config::Args;
use monitor::run_monitor;

#[tokio::main]
async fn main() {
    let args = Args::parse();
    run_monitor(args).await;
}
