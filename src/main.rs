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
    /// Door sensor API URL
    #[arg(long)]
    api_url: String,

    /// Check interval in seconds
    #[arg(long, default_value = "5")]
    check_interval: u64,

    /// Warning threshold in seconds
    #[arg(long, default_value = "15")]
    warning_threshold: u64,

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
    // Debug print all available arguments
    println!("SMS Function Args Debug:");
    println!("  sms_api_username: {:?}", args.sms_api_username);
    println!("  sms_api_password: {:?}", args.sms_api_password.as_ref().map(|_| "[REDACTED]"));
    println!("  sms_from_phone_number: {:?}", args.sms_from_phone_number);
    println!("  sms_to_phone_number: {:?}", args.sms_to_phone_number);
    println!("  message: {:?}", message);


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

        println!("Voip URI: {}", uri);

        let response = client.get(&uri).send().await?;

        println!("SMS Response: {}", response.status().as_str());
        
        if response.status().is_success() {
            println!("SMS sent successfully: {}", message);
        } else {
            eprintln!("Failed to send SMS: HTTP {}", response.status());
        }
    } else {
        println!("SMS args not supplied");
    }
    
    Ok(())
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    
    println!("Door Monitor Starting...");
    println!("API URL: {}", args.api_url);
    println!("Check interval: {} seconds", args.check_interval);
    println!("Warning threshold: {} seconds", args.warning_threshold);
    
    let client = reqwest::Client::new();
    let mut door_opened_time: Option<Instant> = None;
    let mut last_door_state: Option<bool> = None;
    let mut sms_sent = false;
    
    let check_interval = Duration::from_secs(args.check_interval);
    let warning_threshold = Duration::from_secs(args.warning_threshold);
    
    loop {
        match check_door_status(&client, &args.api_url).await {
            Ok(door_status) => {
                let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
                let door_closed = door_status.state;  // TODO: Fix this, unnegate it
                
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
                        // If door is now closed and we had sent an SMS about it being open too long
                        if sms_sent {
                            if let Some(opened_time) = door_opened_time {
                                let total_time_open = opened_time.elapsed();
                                let message = format!("Door is now closed after being open for {:.1} seconds", total_time_open.as_secs_f64());
                                println!("[{}] Sending door closed SMS...", timestamp);
                                if let Err(e) = send_sms(&client, &args, &message).await {
                                    eprintln!("[{}] Failed to send door closed SMS: {}", timestamp, e);
                                }
                            }
                        }
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
                        if time_open >= warning_threshold {
                            println!("[{}] The door has been opened for too long ({:.1} seconds)", 
                                   timestamp, time_open.as_secs_f64());
                            
                            // Send SMS once when threshold is reached
                            if !sms_sent {
                                println!("[{}] Preparing to send SMS...", timestamp);
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
        
        sleep(check_interval).await;
    }
}

async fn check_door_status(client: &reqwest::Client, api_url: &str) -> Result<DoorStatus, Box<dyn std::error::Error>> {
    let response = client.get(api_url).send().await?;
    
    if response.status().is_success() {
        let door_status: DoorStatus = response.json().await?;
        Ok(door_status)
    } else {
        Err(format!("HTTP error: {}", response.status()).into())
    }
}
