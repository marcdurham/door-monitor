use reqwest;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use chrono::Utc;

#[derive(Debug, Deserialize, Serialize)]
struct DoorStatus {
    id: u8,
    state: bool,
}

const API_URL: &str = "http://192.168.1.226/rpc/Input.GetStatus?id=0";
const CHECK_INTERVAL: Duration = Duration::from_secs(5);
const WARNING_THRESHOLD: Duration = Duration::from_secs(15);

#[tokio::main]
async fn main() {
    println!("Door Monitor Starting...");
    
    let client = reqwest::Client::new();
    let mut door_opened_time: Option<Instant> = None;
    let mut last_door_state: Option<bool> = None;
    
    loop {
        match check_door_status(&client).await {
            Ok(door_status) => {
                let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
                let door_closed = door_status.state;
                
                // Check if door state has changed
                if last_door_state != Some(door_closed) {
                    if door_closed {
                        println!("[{}] The door is closed", timestamp);
                        door_opened_time = None;
                    } else {
                        println!("[{}] The door is open", timestamp);
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
