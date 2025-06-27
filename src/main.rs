use reqwest;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use chrono::Utc;
use clap::Parser;

#[derive(Debug, Deserialize, Serialize)]
struct DoorStatus {
    id: u8,
    state: bool,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// SMS API Username for voip.ms
    #[arg(long)]
    sms_api_username: Option<String>,

    /// SMS API Password for voip.ms  
    #[arg(long)]
    sms_api_password: Option<String>,

    /// SMS From Phone Number (DID)
    #[arg(long)]
    sms_from_phone_number: Option<String>,

    /// SMS To Phone Number
    #[arg(long)]
    sms_to_phone_number: Option<String>,
}

const API_URL: &str = "http://192.168.1.226/rpc/Input.GetStatus?id=0";
const CHECK_INTERVAL: Duration = Duration::from_secs(5);
const WARNING_THRESHOLD: Duration = Duration::from_secs(15);

fn play_beep() {
    // Send terminal bell character to make an audible beep
    print!("\x07");
    use std::io::{self, Write};
    io::stdout().flush().unwrap();
}

async fn send_sms(
    client: &reqwest::Client,
    args: &Args,
    message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if let (Some(username), Some(password), Some(from), Some(to)) = (
        &args.sms_api_username,
        &args.sms_api_password,
        &args.sms_from_phone_number,
        &args.sms_to_phone_number,
    ) {
        let uri = format!(
            "https://voip.ms/api/v1/rest.php?api_username={}&api_password={}&method=sendSMS&did={}&dst={}&message={}",
            urlencoding::encode(username),
            urlencoding::encode(password),
            urlencoding::encode(from),
            urlencoding::encode(to),
            urlencoding::encode(message)
        );

        let response = client.get(&uri).send().await?;
        
        if response.status().is_success() {
            println!("SMS sent successfully: {}", message);
        } else {
            eprintln!("Failed to send SMS: HTTP {}", response.status());
        }
    }
    
    Ok(())
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    
    println!("Door Monitor Starting...");
    
    let client = reqwest::Client::new();
    let mut door_opened_time: Option<Instant> = None;
    let mut last_door_state: Option<bool> = None;
    let mut sms_sent = false;
    
    loop {
        match check_door_status(&client).await {
            Ok(door_status) => {
                let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
                let door_closed = door_status.state;
                
                // Always log the current door state
                if door_closed {
                    println!("[{}] The door is closed", timestamp);
                } else {
                    println!("[{}] The door is open", timestamp);
                    // Play beep when door is open
                    play_beep();
                }
                
                // Track when door was opened for timing warnings
                if last_door_state != Some(door_closed) {
                    if door_closed {
                        door_opened_time = None;
                        sms_sent = false; // Reset SMS flag when door closes
                    } else {
                        door_opened_time = Some(Instant::now());
                    }
                    last_door_state = Some(door_closed);
                }
                
                // Check if door has been open too long
                if !door_closed {
                    if let Some(opened_time) = door_opened_time {
                        let time_open = opened_time.elapsed();
                        if time_open >= WARNING_THRESHOLD {
                            println!("[{}] The door has been opened for too long ({:.1} seconds)", 
                                   timestamp, time_open.as_secs_f64());
                            
                            // Send SMS once when threshold is reached
                            if !sms_sent {
                                let message = format!("ALERT: Door has been open for {:.1} seconds", time_open.as_secs_f64());
                                if let Err(e) = send_sms(&client, &args, &message).await {
                                    eprintln!("[{}] Failed to send SMS: {}", timestamp, e);
                                }
                                sms_sent = true;
                            }
                        }
                    }
                }
            }
            Err(e) => {
                let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
                eprintln!("[{}] Error checking door status: {}", timestamp, e);
            }
        }
        
        sleep(CHECK_INTERVAL).await;
    }
}

async fn check_door_status(client: &reqwest::Client) -> Result<DoorStatus, Box<dyn std::error::Error>> {
    let response = client.get(API_URL).send().await?;
    
    if response.status().is_success() {
        let door_status: DoorStatus = response.json().await?;
        Ok(door_status)
    } else {
        Err(format!("HTTP error: {}", response.status()).into())
    }
}
