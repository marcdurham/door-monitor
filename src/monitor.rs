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

/// Door monitoring struct that encapsulates the HTTP client and monitoring state.
/// 
/// This struct manages all door monitoring functionality including:
/// - Making HTTP requests to check door status
/// - Tracking door state changes and durations  
/// - Sending SMS alerts with configurable backoff intervals
/// - Maintaining monitoring state across check cycles
///
/// The struct owns a `reqwest::Client` for HTTP requests, which is more efficient
/// than creating a new client for each request as it reuses connections.
pub struct DoorMonitor {
    client: reqwest::Client,
    state: MonitorState,
}

impl DoorMonitor {
    /// Creates a new DoorMonitor with a fresh HTTP client and initial state.
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            state: MonitorState::new(),
        }
    }

    pub async fn run(&mut self, args: Args) {
        println!("Door Monitor Starting...");
        println!("API URL: {}", args.api_url);
        println!("Check interval: {} seconds", args.check_interval);
        println!("Warning threshold: {} seconds", args.warning_threshold);

        let check_interval = Duration::from_secs(args.check_interval);
        let warning_threshold = Duration::from_secs(args.warning_threshold);
        
        loop {
            match check_door_status(&self.client, &args.api_url).await {
                Ok(door_status) => {
                    self.handle_door_status(&door_status, &args, warning_threshold).await;
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
        &mut self,
        door_status: &DoorStatus,
        args: &Args,
        warning_threshold: Duration,
    ) {
        let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
        let door_closed = door_status.state;
        
        // Always log the current door state
        if door_closed {
            if let Some(closed_time) = self.state.door_closed_time {
                let closed_duration = closed_time.elapsed();
                println!("[{}] The door is closed (closed for {})", timestamp, format_duration(closed_duration));
            } else {
                println!("[{}] The door is closed", timestamp);
            }
        } else {
            if let Some(opened_time) = self.state.door_opened_time {
                let open_duration = opened_time.elapsed();
                println!("[{}] The door is open (open for {})", timestamp, format_duration(open_duration));
            } else {
                println!("[{}] The door is open", timestamp);
            }
            play_beep();
        }
        
        // Track when door state changes
        if self.state.last_door_state != Some(door_closed) {
            self.handle_door_state_change(door_closed, args, &timestamp).await;
            self.state.last_door_state = Some(door_closed);
        }
        
        // Check if door has been open too long
        if !door_closed {
            self.handle_door_open_too_long(args, warning_threshold, &timestamp).await;
        }
    }

    async fn handle_door_state_change(
        &mut self,
        door_closed: bool,
        args: &Args,
        timestamp: &str,
    ) {
        if door_closed {
            // Door just closed
            if self.state.sms_sent {
                if let Some(opened_time) = self.state.door_opened_time {
                    let total_time_open = opened_time.elapsed();
                    let message = format!("Door is now closed after being open for {}", format_duration(total_time_open));
                    println!("[{}] Sending door closed SMS...", timestamp);
                    if let Err(e) = send_sms(&self.client, args, &message).await {
                        eprintln!("[{}] Failed to send door closed SMS: {}", timestamp, e);
                    }
                }
            }
            self.state.door_opened_time = None;
            self.state.door_closed_time = Some(Instant::now());
            self.state.reset_sms_state();
        } else {
            // Door just opened
            self.state.door_opened_time = Some(Instant::now());
            self.state.door_closed_time = None;
        }
    }

    async fn handle_door_open_too_long(
        &mut self,
        args: &Args,
        warning_threshold: Duration,
        timestamp: &str,
    ) {
        if let Some(opened_time) = self.state.door_opened_time {
            let time_open = opened_time.elapsed();
            if time_open >= warning_threshold {
                println!("[{}] The door has been opened for too long ({})", 
                       timestamp, format_duration(time_open));
                
                // SMS logic with backoff if enabled
                if args.sms_backoff() {
                    self.handle_sms_with_backoff(args, time_open, timestamp).await;
                } else {
                    self.handle_single_sms(args, time_open, timestamp).await;
                }
            }
        }
    }

    async fn handle_sms_with_backoff(
        &mut self,
        args: &Args,
        time_open: Duration,
        timestamp: &str,
    ) {
        // SMS backoff intervals: 5, 15, 30, 60 minutes, then every 60 minutes
        let sms_intervals = vec![
            Duration::from_secs(5 * 60),   // 5 minutes
            Duration::from_secs(15 * 60),  // 15 minutes
            Duration::from_secs(30 * 60),  // 30 minutes
            Duration::from_secs(60 * 60),  // 60 minutes
        ];

        let should_send_sms = if !self.state.sms_sent {
            // First SMS - send immediately when threshold is reached
            true
        } else if let Some(last_sms) = self.state.last_sms_time {
            // Determine next interval based on backoff index
            let next_interval = if self.state.sms_backoff_index < sms_intervals.len() {
                sms_intervals[self.state.sms_backoff_index]
            } else {
                Duration::from_secs(60 * 60) // Every 60 minutes after the initial intervals
            };
            
            last_sms.elapsed() >= next_interval
        } else {
            false
        };
        
        if should_send_sms {
            println!("[{}] Preparing to send SMS (backoff index: {})...", timestamp, self.state.sms_backoff_index);
            let message = if !self.state.sms_sent {
                format!("ALERT: Door has been open for {}", format_duration(time_open))
            } else {
                format!("REMINDER: Door still open for {}", format_duration(time_open))
            };
            
            if let Err(e) = send_sms(&self.client, args, &message).await {
                eprintln!("[{}] Failed to send SMS: {}", timestamp, e);
            }
            
            self.state.sms_sent = true;
            self.state.last_sms_time = Some(Instant::now());
            self.state.sms_backoff_index += 1;
        }
    }

    async fn handle_single_sms(
        &mut self,
        args: &Args,
        time_open: Duration,
        timestamp: &str,
    ) {
        if !self.state.sms_sent {
            println!("[{}] Preparing to send SMS...", timestamp);
            let message = format!("ALERT: Door has been open for {}", format_duration(time_open));
            if let Err(e) = send_sms(&self.client, args, &message).await {
                eprintln!("[{}] Failed to send SMS: {}", timestamp, e);
            }
            self.state.sms_sent = true;
        }
    }
}

pub async fn run_monitor(args: Args) {
    let mut monitor = DoorMonitor::new();
    monitor.run(args).await;
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
    fn test_door_monitor_new() {
        let monitor = DoorMonitor::new();
        assert!(monitor.state.door_opened_time.is_none());
        assert!(monitor.state.door_closed_time.is_none());
        assert!(monitor.state.last_door_state.is_none());
        assert!(!monitor.state.sms_sent);
        assert_eq!(monitor.state.sms_backoff_index, 0);
        assert!(monitor.state.last_sms_time.is_none());
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
        
        let mut monitor = DoorMonitor::new();
        let args = Args::try_parse_from(&["test", "--api-url", "http://test.com"]).unwrap();
        let timestamp = "2025-06-28 14:30:15 UTC";

        // Simulate door opening
        monitor.handle_door_state_change(false, &args, timestamp).await;

        assert!(monitor.state.door_opened_time.is_some());
        assert!(monitor.state.door_closed_time.is_none());
    }

    #[tokio::test]
    async fn test_handle_door_state_change_door_closes() {
        use crate::config::Args;
        use clap::Parser;
        
        let mut monitor = DoorMonitor::new();
        monitor.state.door_opened_time = Some(Instant::now());
        let args = Args::try_parse_from(&["test", "--api-url", "http://test.com"]).unwrap();
        let timestamp = "2025-06-28 14:30:15 UTC";

        // Simulate door closing
        monitor.handle_door_state_change(true, &args, timestamp).await;

        assert!(monitor.state.door_opened_time.is_none());
        assert!(monitor.state.door_closed_time.is_some());
        assert!(!monitor.state.sms_sent);
        assert_eq!(monitor.state.sms_backoff_index, 0);
    }
}
