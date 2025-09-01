use clap::Parser;

mod config;
mod door;
mod audio;
mod utils;
mod sms;
mod telegram;
mod monitor;

use config::Args;
use monitor::run_monitor;
use monitor::send_telegram_test_message;

#[tokio::main]
async fn main() {
    let args = Args::parse();
    if args.telegram_test {
        send_telegram_test_message(args).await;
    } else {
        run_monitor(args).await;
    }
}
