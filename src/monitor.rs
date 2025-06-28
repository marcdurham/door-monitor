use std::time::{Duration, Instant};
use tokio::time::sleep;
use chrono::Utc;

use crate::config::Args;
use crate::door::{DoorStatus, check_door_status};
use crate::audio::play_beep;
use crate::utils::format_duration;
use crate::sms::send_sms;

pub struct MonitorState {
    pub door_opened_time: Option<Instant>,
    pub door_closed_time: Option<Instant>,
    pub last_door_state: Option<bool>,
    pub sms_sent: bool,
    pub sms_backoff_index: usize,
    pub last_sms_time: Option<Instant>,
}

impl MonitorState {
    pub fn new() -> Self {
        Self {
            door_opened_time: None,
            door_closed_time: None,
            last_door_state: None,
            sms_sent: false,
            sms_backoff_index: 0,
            last_sms_time: None,
        }
    }

    pub fn reset_sms_state(&mut self) {
        self.sms_sent = false;
        self.sms_backoff_index = 0;
        self.last_sms_time = None;
    }
}

pub async fn run_monitor(args: Args) {
    println!("Door Monitor Starting...");
    println!("API URL: {}", args.api_url);
    println!("Check interval: {} seconds", args.check_interval);
    println!("Warning threshold: {} seconds", args.warning_threshold);
    
    let client = reqwest::Client::new();
    let mut state = MonitorState::new();

    let check_interval = Duration::from_secs(args.check_interval);
    let warning_threshold = Duration::from_secs(args.warning_threshold);
    
    loop {
        match check_door_status(&client, &args.api_url).await {
            Ok(door_status) => {
                handle_door_status(&door_status, &mut state, &args, &client, warning_threshold).await;
            }
            Err(e) => {
                let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
                eprintln!("[{}] Error checking door status: {}", timestamp, e);
            }
        }
        
        sleep(check_interval).await;
    }
}

async fn handle_door_status(
    door_status: &DoorStatus,
    state: &mut MonitorState,
    args: &Args,
    client: &reqwest::Client,
    warning_threshold: Duration,
) {
    // SMS backoff intervals: 5, 15, 30, 60 minutes, then every 60 minutes
    let sms_intervals = vec![
        Duration::from_secs(5 * 60),   // 5 minutes
        Duration::from_secs(15 * 60),  // 15 minutes
        Duration::from_secs(30 * 60),  // 30 minutes
        Duration::from_secs(60 * 60),  // 60 minutes
    ];
    
    let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
    let door_closed = door_status.state;
    
    // Always log the current door state
    if door_closed {
        if let Some(closed_time) = state.door_closed_time {
            let closed_duration = closed_time.elapsed();
            println!("[{}] The door is closed (closed for {})", timestamp, format_duration(closed_duration));
        } else {
            println!("[{}] The door is closed", timestamp);
        }
    } else {
        if let Some(opened_time) = state.door_opened_time {
            let open_duration = opened_time.elapsed();
            println!("[{}] The door is open (open for {})", timestamp, format_duration(open_duration));
        } else {
            println!("[{}] The door is open", timestamp);
        }
        play_beep();
    }
    
    // Track when door state changes
    if state.last_door_state != Some(door_closed) {
        handle_door_state_change(door_closed, state, args, client, &timestamp).await;
        state.last_door_state = Some(door_closed);
    }
    
    // Check if door has been open too long
    if !door_closed {
        handle_door_open_too_long(state, args, client, &sms_intervals, warning_threshold, &timestamp).await;
    }
}

async fn handle_door_state_change(
    door_closed: bool,
    state: &mut MonitorState,
    args: &Args,
    client: &reqwest::Client,
    timestamp: &str,
) {
    if door_closed {
        // Door just closed
        if state.sms_sent {
            if let Some(opened_time) = state.door_opened_time {
                let total_time_open = opened_time.elapsed();
                let message = format!("Door is now closed after being open for {}", format_duration(total_time_open));
                println!("[{}] Sending door closed SMS...", timestamp);
                if let Err(e) = send_sms(client, args, &message).await {
                    eprintln!("[{}] Failed to send door closed SMS: {}", timestamp, e);
                }
            }
        }
        state.door_opened_time = None;
        state.door_closed_time = Some(Instant::now());
        state.reset_sms_state();
    } else {
        // Door just opened
        state.door_opened_time = Some(Instant::now());
        state.door_closed_time = None;
    }
}

async fn handle_door_open_too_long(
    state: &mut MonitorState,
    args: &Args,
    client: &reqwest::Client,
    sms_intervals: &[Duration],
    warning_threshold: Duration,
    timestamp: &str,
) {
    if let Some(opened_time) = state.door_opened_time {
        let time_open = opened_time.elapsed();
        if time_open >= warning_threshold {
            println!("[{}] The door has been opened for too long ({})", 
                   timestamp, format_duration(time_open));
            
            // SMS logic with backoff if enabled
            if args.sms_backoff() {
                handle_sms_with_backoff(state, args, client, sms_intervals, time_open, timestamp).await;
            } else {
                handle_single_sms(state, args, client, time_open, timestamp).await;
            }
        }
    }
}

async fn handle_sms_with_backoff(
    state: &mut MonitorState,
    args: &Args,
    client: &reqwest::Client,
    sms_intervals: &[Duration],
    time_open: Duration,
    timestamp: &str,
) {
    let should_send_sms = if !state.sms_sent {
        // First SMS - send immediately when threshold is reached
        true
    } else if let Some(last_sms) = state.last_sms_time {
        // Determine next interval based on backoff index
        let next_interval = if state.sms_backoff_index < sms_intervals.len() {
            sms_intervals[state.sms_backoff_index]
        } else {
            Duration::from_secs(60 * 60) // Every 60 minutes after the initial intervals
        };
        
        last_sms.elapsed() >= next_interval
    } else {
        false
    };
    
    if should_send_sms {
        println!("[{}] Preparing to send SMS (backoff index: {})...", timestamp, state.sms_backoff_index);
        let message = if !state.sms_sent {
            format!("ALERT: Door has been open for {}", format_duration(time_open))
        } else {
            format!("REMINDER: Door still open for {}", format_duration(time_open))
        };
        
        if let Err(e) = send_sms(client, args, &message).await {
            eprintln!("[{}] Failed to send SMS: {}", timestamp, e);
        }
        
        state.sms_sent = true;
        state.last_sms_time = Some(Instant::now());
        state.sms_backoff_index += 1;
    }
}

async fn handle_single_sms(
    state: &mut MonitorState,
    args: &Args,
    client: &reqwest::Client,
    time_open: Duration,
    timestamp: &str,
) {
    if !state.sms_sent {
        println!("[{}] Preparing to send SMS...", timestamp);
        let message = format!("ALERT: Door has been open for {}", format_duration(time_open));
        if let Err(e) = send_sms(client, args, &message).await {
            eprintln!("[{}] Failed to send SMS: {}", timestamp, e);
        }
        state.sms_sent = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_monitor_state_new() {
        let state = MonitorState::new();
        assert!(state.door_opened_time.is_none());
        assert!(state.door_closed_time.is_none());
        assert!(state.last_door_state.is_none());
        assert!(!state.sms_sent);
        assert_eq!(state.sms_backoff_index, 0);
        assert!(state.last_sms_time.is_none());
    }

    #[test]
    fn test_monitor_state_reset_sms_state() {
        let mut state = MonitorState::new();
        state.sms_sent = true;
        state.sms_backoff_index = 3;
        state.last_sms_time = Some(Instant::now());

        state.reset_sms_state();

        assert!(!state.sms_sent);
        assert_eq!(state.sms_backoff_index, 0);
        assert!(state.last_sms_time.is_none());
    }

    #[test]
    fn test_sms_intervals() {
        let sms_intervals = vec![
            Duration::from_secs(5 * 60),   // 5 minutes
            Duration::from_secs(15 * 60),  // 15 minutes
            Duration::from_secs(30 * 60),  // 30 minutes
            Duration::from_secs(60 * 60),  // 60 minutes
        ];

        assert_eq!(sms_intervals[0], Duration::from_secs(300));
        assert_eq!(sms_intervals[1], Duration::from_secs(900));
        assert_eq!(sms_intervals[2], Duration::from_secs(1800));
        assert_eq!(sms_intervals[3], Duration::from_secs(3600));
    }

    #[tokio::test]
    async fn test_handle_door_state_change_door_opens() {
        use crate::config::Args;
        use clap::Parser;
        
        let mut state = MonitorState::new();
        let args = Args::try_parse_from(&["test", "--api-url", "http://test.com"]).unwrap();
        let client = reqwest::Client::new();
        let timestamp = "2025-06-28 14:30:15 UTC";

        // Simulate door opening
        handle_door_state_change(false, &mut state, &args, &client, timestamp).await;

        assert!(state.door_opened_time.is_some());
        assert!(state.door_closed_time.is_none());
    }

    #[tokio::test]
    async fn test_handle_door_state_change_door_closes() {
        use crate::config::Args;
        use clap::Parser;
        
        let mut state = MonitorState::new();
        state.door_opened_time = Some(Instant::now());
        let args = Args::try_parse_from(&["test", "--api-url", "http://test.com"]).unwrap();
        let client = reqwest::Client::new();
        let timestamp = "2025-06-28 14:30:15 UTC";

        // Simulate door closing
        handle_door_state_change(true, &mut state, &args, &client, timestamp).await;

        assert!(state.door_opened_time.is_none());
        assert!(state.door_closed_time.is_some());
        assert!(!state.sms_sent);
        assert_eq!(state.sms_backoff_index, 0);
    }
}
