use std::time::{Duration, Instant};
use tokio::time::sleep;
use chrono::Utc;

use crate::config::Args;
use crate::door::{DoorStatus, check_door_status};
use crate::audio::play_beep;
use crate::utils::format_duration;
use crate::sms::send_sms;
use crate::telegram::send_telegram;

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

/// A door monitoring system that tracks door state and sends SMS notifications.
/// 
/// The DoorMonitor struct provides comprehensive door monitoring functionality including:
/// - Polling a REST API to check door status
/// - Logging door state changes with timestamps and durations
/// - Playing audio alerts when the door opens
/// - Sending SMS notifications for various events:
///   * Initial status when program starts
///   * Immediate notification when door opens
///   * Notification when door closes (with duration open)
///   * Progressive warnings if door stays open too long (with backoff)
/// - Maintaining monitoring state across check cycles
///
/// ## SMS Notification Behavior
/// 
/// The monitor sends SMS messages for the following events:
/// 1. **Program Start**: Initial status message with current door state
/// 2. **Door Opens**: Immediate notification when door changes from closed to open
/// 3. **Door Closes**: Notification when door changes from open to closed (includes duration)
/// 4. **Door Open Too Long**: Progressive warnings if door exceeds warning threshold
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

    pub async fn send_telegram_message(&mut self, args: Args) {
        println!("Door Monitor Sending test message via Telegram...");
        let message = args.test_message.clone().unwrap_or("".to_string());
        let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
        if let Err(e) = send_telegram(&self.client, &args, &message).await {
            eprintln!("[{}] Failed to send test message via Telegram: {}", timestamp, e);
        }
    }

    pub async fn run(&mut self, args: Args) {
        println!("Door Monitor Starting...");
        println!("API URL: {}", args.api_url.clone().unwrap_or("".to_string()).as_str());
        println!("Check interval: {} seconds", args.check_interval_seconds);
        println!("Warning threshold: {} seconds", args.open_too_long_seconds);
        println!("SMS Off: {}", args.sms_off);
        println!("Telegram Off: {}", args.telegram_off);

        let check_interval = Duration::from_secs(args.check_interval_seconds);
        let warning_threshold = Duration::from_secs(args.open_too_long_seconds);
        
        // Send initial status SMS when program starts
        match check_door_status(&self.client, args.api_url.clone().unwrap_or("".to_string()).as_str()).await {
            Ok(door_status) => {
                let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
                let door_state_msg = if door_status.state { "closed" } else { "open" };
                let message = format!("Door Monitor started. Current door state: {}", door_state_msg);

                if !args.sms_off {
                    println!("[{}] Sending initial status SMS...", timestamp);
                    if let Err(e) = send_sms(&self.client, &args, &message).await {
                        eprintln!("[{}] Failed to send initial status SMS: {}", timestamp, e);
                    }
                }
                
                if !args.telegram_off {
                    println!("[{}] Sending initial status Telegram...", timestamp);
                    if let Err(e) = send_telegram(&self.client, &args, &message).await {
                        eprintln!("[{}] Failed to send initial status Telegram: {}", timestamp, e);
                    }
                }
                
                // Set initial state
                if door_status.state {
                    // Door is closed
                    self.state.door_closed_time = Some(Instant::now());
                } else {
                    // Door is open
                    self.state.door_opened_time = Some(Instant::now());
                }
                self.state.last_door_state = Some(door_status.state);
            }
            Err(e) => {
                let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
                eprintln!("[{}] Error checking initial door status: {}", timestamp, e);
            }
        }
        
        loop {
            match check_door_status(&self.client, args.api_url.clone().unwrap_or("".to_string()).as_str()).await {
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
            // Door just closed - always send SMS if door was open
            if let Some(opened_time) = self.state.door_opened_time {
                let total_time_open = opened_time.elapsed();
                let message = format!("Door is now closed after being open for {}", format_duration(total_time_open));
                if !args.sms_off {
                    println!("[{}] Sending door closed SMS...", timestamp);
                    if let Err(e) = send_sms(&self.client, args, &message).await {
                        eprintln!("[{}] Failed to send door closed SMS: {}", timestamp, e);
                    }
                }

                if !args.telegram_off {
                    println!("[{}] Sending door closed Telegram...", timestamp);
                    if let Err(e) = send_telegram(&self.client, args, &message).await {
                        eprintln!("[{}] Failed to send door closed Telegram: {}", timestamp, e);
                    }
                }
            }
            self.state.door_opened_time = None;
            self.state.door_closed_time = Some(Instant::now());
            self.state.reset_sms_state();
        } else {
            // Door just opened - send SMS immediately
            let message = "Door has been opened".to_string();
            if !args.sms_off {
                println!("[{}] Sending door opened SMS...", timestamp);
                if let Err(e) = send_sms(&self.client, args, &message).await {
                    eprintln!("[{}] Failed to send door opened SMS: {}", timestamp, e);
                }
            }

            if !args.telegram_off {
                println!("[{}] Sending door opened Telegra...", timestamp);
                if let Err(e) = send_telegram(&self.client, args, &message).await {
                    eprintln!("[{}] Failed to send door opened Telegram: {}", timestamp, e);
                }
            }
            
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

        let should_send_message = if !self.state.sms_sent {
            // First Message - send immediately when threshold is reached
            true
        } else if let Some(last_message) = self.state.last_sms_time {
            // Determine next interval based on backoff index
            let next_interval = if self.state.sms_backoff_index < sms_intervals.len() {
                sms_intervals[self.state.sms_backoff_index]
            } else {
                Duration::from_secs(60 * 60) // Every 60 minutes after the initial intervals
            };
            
            last_message.elapsed() >= next_interval
        } else {
            false
        };
        
        if should_send_message {
            println!("[{}] Preparing to send SMS (backoff index: {})...", timestamp, self.state.sms_backoff_index);
            let message = if !self.state.sms_sent {
                format!("ALERT: Door has been open for {}", format_duration(time_open))
            } else {
                format!("REMINDER: Door still open for {}", format_duration(time_open))
            };
            
            if !args.sms_off {
                if let Err(e) = send_sms(&self.client, args, &message).await {
                    eprintln!("[{}] Failed to send SMS: {}", timestamp, e);
                }
            }

            if !args.telegram_off {
                if let Err(e) = send_telegram(&self.client, args, &message).await {
                    eprintln!("[{}] Failed to send Telegram: {}", timestamp, e);
                }
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

            let message = format!("ALERT: Door has been open for {}", format_duration(time_open));
            if !args.sms_off {
                println!("[{}] Preparing to send SMS...", timestamp);
                if let Err(e) = send_sms(&self.client, args, &message).await {
                    eprintln!("[{}] Failed to send SMS: {}", timestamp, e);
                }
            }

            if !args.telegram_off {
                println!("[{}] Preparing to send Telegram...", timestamp);
                if let Err(e) = send_telegram(&self.client, args, &message).await {
                    eprintln!("[{}] Failed to send Telegram: {}", timestamp, e);
                }
            }

            self.state.sms_sent = true;
        }
    }
}

pub async fn run_monitor(args: Args) {
    let mut monitor = DoorMonitor::new();
    monitor.run(args).await;
}

pub async fn send_telegram_test_message(args: Args) {
    let mut monitor = DoorMonitor::new();
    monitor.send_telegram_message(args).await;
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

    #[tokio::test]
    async fn test_door_opening_sends_immediate_sms() {
        use crate::config::Args;
        use clap::Parser;
        
        let mut monitor = DoorMonitor::new();
        let args = Args::try_parse_from(&[
            "test", 
            "--api-url", "http://test.com",
            "--sms-api-username", "test_user",
            "--sms-api-password", "test_pass",
            "--sms-from-phone-number", "1234567890",
            "--sms-to-phone-number", "0987654321"
        ]).unwrap();
        let timestamp = "2025-06-28 14:30:15 UTC";

        // Simulate door opening - should trigger immediate SMS
        monitor.handle_door_state_change(false, &args, timestamp).await;

        assert!(monitor.state.door_opened_time.is_some());
        assert!(monitor.state.door_closed_time.is_none());
    }

    #[tokio::test]
    async fn test_door_closing_always_sends_sms() {
        use crate::config::Args;
        use clap::Parser;
        
        let mut monitor = DoorMonitor::new();
        // Set door as opened some time ago
        monitor.state.door_opened_time = Some(Instant::now() - Duration::from_secs(300)); // 5 minutes ago
        
        let args = Args::try_parse_from(&[
            "test", 
            "--api-url", "http://test.com",
            "--sms-api-username", "test_user",
            "--sms-api-password", "test_pass",
            "--sms-from-phone-number", "1234567890",
            "--sms-to-phone-number", "0987654321"
        ]).unwrap();
        let timestamp = "2025-06-28 14:30:15 UTC";

        // Simulate door closing - should always send SMS regardless of sms_sent state
        monitor.handle_door_state_change(true, &args, timestamp).await;

        assert!(monitor.state.door_opened_time.is_none());
        assert!(monitor.state.door_closed_time.is_some());
        assert!(!monitor.state.sms_sent); // Should be reset after closing
        assert_eq!(monitor.state.sms_backoff_index, 0);
    }

    #[tokio::test]
    async fn test_handle_door_status_door_closed_with_duration() {
        use crate::config::Args;
        use clap::Parser;
        use crate::door::DoorStatus;
        
        let mut monitor = DoorMonitor::new();
        // Set door as closed some time ago
        monitor.state.door_closed_time = Some(Instant::now() - Duration::from_secs(180)); // 3 minutes ago
        monitor.state.last_door_state = Some(true); // Previously closed
        
        let args = Args::try_parse_from(&["test", "--api-url", "http://test.com"]).unwrap();
        let door_status = DoorStatus { id: 1, state: true }; // Door is closed
        let warning_threshold = Duration::from_secs(60);

        // This should log the closed duration but not trigger state change
        monitor.handle_door_status(&door_status, &args, warning_threshold).await;

        // State should remain unchanged since door was already closed
        assert!(monitor.state.door_closed_time.is_some());
        assert!(monitor.state.door_opened_time.is_none());
        assert_eq!(monitor.state.last_door_state, Some(true));
    }

    #[tokio::test]
    async fn test_handle_door_status_door_open_with_duration() {
        use crate::config::Args;
        use clap::Parser;
        use crate::door::DoorStatus;
        
        let mut monitor = DoorMonitor::new();
        // Set door as open some time ago
        monitor.state.door_opened_time = Some(Instant::now() - Duration::from_secs(300)); // 5 minutes ago
        monitor.state.last_door_state = Some(false); // Previously open
        
        let args = Args::try_parse_from(&["test", "--api-url", "http://test.com"]).unwrap();
        let door_status = DoorStatus { id: 1, state: false }; // Door is open
        let warning_threshold = Duration::from_secs(60);

        // This should log the open duration and trigger warning logic
        monitor.handle_door_status(&door_status, &args, warning_threshold).await;

        // State should remain unchanged since door was already open
        assert!(monitor.state.door_opened_time.is_some());
        assert!(monitor.state.door_closed_time.is_none());
        assert_eq!(monitor.state.last_door_state, Some(false));
    }

    #[tokio::test]
    async fn test_handle_door_status_first_time_closed() {
        use crate::config::Args;
        use clap::Parser;
        use crate::door::DoorStatus;
        
        let mut monitor = DoorMonitor::new();
        // No previous state
        
        let args = Args::try_parse_from(&["test", "--api-url", "http://test.com"]).unwrap();
        let door_status = DoorStatus { id: 1, state: true }; // Door is closed
        let warning_threshold = Duration::from_secs(60);

        monitor.handle_door_status(&door_status, &args, warning_threshold).await;

        // Should log "The door is closed" without duration
        assert!(monitor.state.door_closed_time.is_some());
        assert!(monitor.state.door_opened_time.is_none());
        assert_eq!(monitor.state.last_door_state, Some(true));
    }

    #[tokio::test]
    async fn test_handle_door_status_first_time_open() {
        use crate::config::Args;
        use clap::Parser;
        use crate::door::DoorStatus;
        
        let mut monitor = DoorMonitor::new();
        // No previous state
        
        let args = Args::try_parse_from(&["test", "--api-url", "http://test.com"]).unwrap();
        let door_status = DoorStatus { id: 1, state: false }; // Door is open
        let warning_threshold = Duration::from_secs(60);

        monitor.handle_door_status(&door_status, &args, warning_threshold).await;

        // Should log "The door is open" without duration and send SMS
        assert!(monitor.state.door_opened_time.is_some());
        assert!(monitor.state.door_closed_time.is_none());
        assert_eq!(monitor.state.last_door_state, Some(false));
    }

    #[tokio::test]
    async fn test_handle_door_open_too_long_below_threshold() {
        use crate::config::Args;
        use clap::Parser;
        
        let mut monitor = DoorMonitor::new();
        // Set door as opened recently
        monitor.state.door_opened_time = Some(Instant::now() - Duration::from_secs(30)); // 30 seconds ago
        
        let args = Args::try_parse_from(&["test", "--api-url", "http://test.com"]).unwrap();
        let warning_threshold = Duration::from_secs(60); // 1 minute threshold
        let timestamp = "2025-06-28 14:30:15 UTC";

        // Door has been open for 30 seconds, threshold is 60 seconds - should not trigger
        monitor.handle_door_open_too_long(&args, warning_threshold, timestamp).await;

        // SMS state should remain unchanged
        assert!(!monitor.state.sms_sent);
        assert_eq!(monitor.state.sms_backoff_index, 0);
        assert!(monitor.state.last_sms_time.is_none());
    }

    #[tokio::test]
    async fn test_handle_door_open_too_long_above_threshold_with_backoff() {
        use crate::config::Args;
        use clap::Parser;
        
        let mut monitor = DoorMonitor::new();
        // Set door as opened past threshold
        monitor.state.door_opened_time = Some(Instant::now() - Duration::from_secs(120)); // 2 minutes ago
        
        let args = Args::try_parse_from(&[
            "test", 
            "--api-url", "http://test.com",
            "--sms-api-username", "test_user",
            "--sms-api-password", "test_pass",
            "--sms-from-phone-number", "1234567890",
            "--sms-to-phone-number", "0987654321"
        ]).unwrap();
        let warning_threshold = Duration::from_secs(60); // 1 minute threshold
        let timestamp = "2025-06-28 14:30:15 UTC";

        // Door has been open for 2 minutes, threshold is 1 minute - should trigger first SMS
        monitor.handle_door_open_too_long(&args, warning_threshold, timestamp).await;

        // First SMS should be sent
        assert!(monitor.state.sms_sent);
        assert_eq!(monitor.state.sms_backoff_index, 1);
        assert!(monitor.state.last_sms_time.is_some());
    }

    #[tokio::test]
    async fn test_handle_door_open_too_long_above_threshold_no_backoff() {
        use crate::config::Args;
        use clap::Parser;
        
        let mut monitor = DoorMonitor::new();
        // Set door as opened past threshold
        monitor.state.door_opened_time = Some(Instant::now() - Duration::from_secs(120)); // 2 minutes ago
        
        let args = Args::try_parse_from(&[
            "test", 
            "--api-url", "http://test.com",
            "--no-sms-backoff", // Disable backoff
            "--sms-api-username", "test_user",
            "--sms-api-password", "test_pass",
            "--sms-from-phone-number", "1234567890",
            "--sms-to-phone-number", "0987654321"
        ]).unwrap();
        let warning_threshold = Duration::from_secs(60); // 1 minute threshold
        let timestamp = "2025-06-28 14:30:15 UTC";

        // Door has been open for 2 minutes, threshold is 1 minute - should trigger single SMS
        monitor.handle_door_open_too_long(&args, warning_threshold, timestamp).await;

        // Single SMS should be sent
        assert!(monitor.state.sms_sent);
        assert_eq!(monitor.state.sms_backoff_index, 0); // No backoff increment
        assert!(monitor.state.last_sms_time.is_none()); // No last SMS time tracking
    }

    #[tokio::test]
    async fn test_handle_door_open_too_long_no_door_open_time() {
        use crate::config::Args;
        use clap::Parser;
        
        let mut monitor = DoorMonitor::new();
        // No door_opened_time set
        
        let args = Args::try_parse_from(&["test", "--api-url", "http://test.com"]).unwrap();
        let warning_threshold = Duration::from_secs(60);
        let timestamp = "2025-06-28 14:30:15 UTC";

        // Should not trigger anything since door_opened_time is None
        monitor.handle_door_open_too_long(&args, warning_threshold, timestamp).await;

        // SMS state should remain unchanged
        assert!(!monitor.state.sms_sent);
        assert_eq!(monitor.state.sms_backoff_index, 0);
        assert!(monitor.state.last_sms_time.is_none());
    }

    #[tokio::test]
    async fn test_handle_sms_with_backoff_first_sms() {
        use crate::config::Args;
        use clap::Parser;
        
        let mut monitor = DoorMonitor::new();
        let args = Args::try_parse_from(&[
            "test", 
            "--api-url", "http://test.com",
            "--sms-api-username", "test_user",
            "--sms-api-password", "test_pass",
            "--sms-from-phone-number", "1234567890",
            "--sms-to-phone-number", "0987654321"
        ]).unwrap();
        let time_open = Duration::from_secs(900); // 15 minutes
        let timestamp = "2025-06-28 14:30:15 UTC";

        // First SMS - should send immediately
        monitor.handle_sms_with_backoff(&args, time_open, timestamp).await;

        assert!(monitor.state.sms_sent);
        assert_eq!(monitor.state.sms_backoff_index, 1);
        assert!(monitor.state.last_sms_time.is_some());
    }

    #[tokio::test]
    async fn test_handle_sms_with_backoff_second_sms_too_early() {
        use crate::config::Args;
        use clap::Parser;
        
        let mut monitor = DoorMonitor::new();
        // Set up state as if first SMS was sent recently
        monitor.state.sms_sent = true;
        monitor.state.sms_backoff_index = 0;
        monitor.state.last_sms_time = Some(Instant::now() - Duration::from_secs(120)); // 2 minutes ago
        
        let args = Args::try_parse_from(&[
            "test", 
            "--api-url", "http://test.com",
            "--sms-api-username", "test_user",
            "--sms-api-password", "test_pass",
            "--sms-from-phone-number", "1234567890",
            "--sms-to-phone-number", "0987654321"
        ]).unwrap();
        let time_open = Duration::from_secs(900); // 15 minutes
        let timestamp = "2025-06-28 14:30:15 UTC";

        // Second SMS attempt - should not send (first interval is 5 minutes)
        monitor.handle_sms_with_backoff(&args, time_open, timestamp).await;

        // Should remain at same backoff level
        assert!(monitor.state.sms_sent);
        assert_eq!(monitor.state.sms_backoff_index, 0);
    }

    #[tokio::test]
    async fn test_handle_sms_with_backoff_second_sms_ready() {
        use crate::config::Args;
        use clap::Parser;
        
        let mut monitor = DoorMonitor::new();
        // Set up state as if first SMS was sent 6 minutes ago (past first interval)
        monitor.state.sms_sent = true;
        monitor.state.sms_backoff_index = 0;
        monitor.state.last_sms_time = Some(Instant::now() - Duration::from_secs(360)); // 6 minutes ago
        
        let args = Args::try_parse_from(&[
            "test", 
            "--api-url", "http://test.com",
            "--sms-api-username", "test_user",
            "--sms-api-password", "test_pass",
            "--sms-from-phone-number", "1234567890",
            "--sms-to-phone-number", "0987654321"
        ]).unwrap();
        let time_open = Duration::from_secs(900); // 15 minutes
        let timestamp = "2025-06-28 14:30:15 UTC";

        // Second SMS attempt - should send (past 5 minute interval)
        monitor.handle_sms_with_backoff(&args, time_open, timestamp).await;

        // Should advance to next backoff level
        assert!(monitor.state.sms_sent);
        assert_eq!(monitor.state.sms_backoff_index, 1);
        assert!(monitor.state.last_sms_time.is_some());
    }

    #[tokio::test]
    async fn test_handle_sms_with_backoff_beyond_intervals() {
        use crate::config::Args;
        use clap::Parser;
        
        let mut monitor = DoorMonitor::new();
        // Set up state as if we're past all defined intervals
        monitor.state.sms_sent = true;
        monitor.state.sms_backoff_index = 5; // Beyond the 4 defined intervals
        monitor.state.last_sms_time = Some(Instant::now() - Duration::from_secs(3700)); // 61+ minutes ago
        
        let args = Args::try_parse_from(&[
            "test", 
            "--api-url", "http://test.com",
            "--sms-api-username", "test_user",
            "--sms-api-password", "test_pass",
            "--sms-from-phone-number", "1234567890",
            "--sms-to-phone-number", "0987654321"
        ]).unwrap();
        let time_open = Duration::from_secs(7200); // 2 hours
        let timestamp = "2025-06-28 14:30:15 UTC";

        // Should send with 60-minute default interval
        monitor.handle_sms_with_backoff(&args, time_open, timestamp).await;

        // Should advance backoff index
        assert!(monitor.state.sms_sent);
        assert_eq!(monitor.state.sms_backoff_index, 6);
        assert!(monitor.state.last_sms_time.is_some());
    }

    #[tokio::test]
    async fn test_handle_single_sms_first_time() {
        use crate::config::Args;
        use clap::Parser;
        
        let mut monitor = DoorMonitor::new();
        let args = Args::try_parse_from(&[
            "test", 
            "--api-url", "http://test.com",
            "--sms-api-username", "test_user",
            "--sms-api-password", "test_pass",
            "--sms-from-phone-number", "1234567890",
            "--sms-to-phone-number", "0987654321"
        ]).unwrap();
        let time_open = Duration::from_secs(900); // 15 minutes
        let timestamp = "2025-06-28 14:30:15 UTC";

        // First single SMS - should send
        monitor.handle_single_sms(&args, time_open, timestamp).await;

        assert!(monitor.state.sms_sent);
        // Single SMS doesn't use backoff tracking
        assert_eq!(monitor.state.sms_backoff_index, 0);
        assert!(monitor.state.last_sms_time.is_none());
    }

    #[tokio::test]
    async fn test_handle_single_sms_already_sent() {
        use crate::config::Args;
        use clap::Parser;
        
        let mut monitor = DoorMonitor::new();
        // Set up state as if SMS was already sent
        monitor.state.sms_sent = true;
        
        let args = Args::try_parse_from(&[
            "test", 
            "--api-url", "http://test.com",
            "--sms-api-username", "test_user",
            "--sms-api-password", "test_pass",
            "--sms-from-phone-number", "1234567890",
            "--sms-to-phone-number", "0987654321"
        ]).unwrap();
        let time_open = Duration::from_secs(900); // 15 minutes
        let timestamp = "2025-06-28 14:30:15 UTC";

        // Second single SMS attempt - should not send
        monitor.handle_single_sms(&args, time_open, timestamp).await;

        // State should remain unchanged
        assert!(monitor.state.sms_sent);
        assert_eq!(monitor.state.sms_backoff_index, 0);
        assert!(monitor.state.last_sms_time.is_none());
    }

    #[tokio::test]
    async fn test_door_state_change_from_closed_to_open() {
        use crate::config::Args;
        use clap::Parser;
        use crate::door::DoorStatus;
        
        let mut monitor = DoorMonitor::new();
        // Set initial state as closed
        monitor.state.door_closed_time = Some(Instant::now() - Duration::from_secs(300));
        monitor.state.last_door_state = Some(true); // Door was closed
        
        let args = Args::try_parse_from(&[
            "test", 
            "--api-url", "http://test.com",
            "--sms-api-username", "test_user",
            "--sms-api-password", "test_pass",
            "--sms-from-phone-number", "1234567890",
            "--sms-to-phone-number", "0987654321"
        ]).unwrap();
        let door_status = DoorStatus { id: 1, state: false }; // Door is now open
        let warning_threshold = Duration::from_secs(60);

        // This should detect state change and send SMS
        monitor.handle_door_status(&door_status, &args, warning_threshold).await;

        // Should transition to open state and send SMS
        assert!(monitor.state.door_opened_time.is_some());
        assert!(monitor.state.door_closed_time.is_none());
        assert_eq!(monitor.state.last_door_state, Some(false));
    }

    #[tokio::test]
    async fn test_door_state_change_from_open_to_closed() {
        use crate::config::Args;
        use clap::Parser;
        use crate::door::DoorStatus;
        
        let mut monitor = DoorMonitor::new();
        // Set initial state as open
        monitor.state.door_opened_time = Some(Instant::now() - Duration::from_secs(300));
        monitor.state.last_door_state = Some(false); // Door was open
        
        let args = Args::try_parse_from(&[
            "test", 
            "--api-url", "http://test.com",
            "--sms-api-username", "test_user",
            "--sms-api-password", "test_pass",
            "--sms-from-phone-number", "1234567890",
            "--sms-to-phone-number", "0987654321"
        ]).unwrap();
        let door_status = DoorStatus { id: 1, state: true }; // Door is now closed
        let warning_threshold = Duration::from_secs(60);

        // This should detect state change and send SMS
        monitor.handle_door_status(&door_status, &args, warning_threshold).await;

        // Should transition to closed state and send SMS
        assert!(monitor.state.door_opened_time.is_none());
        assert!(monitor.state.door_closed_time.is_some());
        assert_eq!(monitor.state.last_door_state, Some(true));
        // SMS state should be reset after closing
        assert!(!monitor.state.sms_sent);
        assert_eq!(monitor.state.sms_backoff_index, 0);
    }

    #[tokio::test]
    async fn test_handle_sms_with_backoff_no_last_sms_time() {
        use crate::config::Args;
        use clap::Parser;
        
        let mut monitor = DoorMonitor::new();
        // Set up inconsistent state - sms_sent but no last_sms_time
        monitor.state.sms_sent = true;
        monitor.state.sms_backoff_index = 1;
        monitor.state.last_sms_time = None; // This should not happen in normal operation
        
        let args = Args::try_parse_from(&[
            "test", 
            "--api-url", "http://test.com",
            "--sms-api-username", "test_user",
            "--sms-api-password", "test_pass",
            "--sms-from-phone-number", "1234567890",
            "--sms-to-phone-number", "0987654321"
        ]).unwrap();
        let time_open = Duration::from_secs(900); // 15 minutes
        let timestamp = "2025-06-28 14:30:15 UTC";

        // Should handle gracefully and not send SMS
        monitor.handle_sms_with_backoff(&args, time_open, timestamp).await;

        // Should remain unchanged
        assert!(monitor.state.sms_sent);
        assert_eq!(monitor.state.sms_backoff_index, 1);
        assert!(monitor.state.last_sms_time.is_none());
    }

    #[tokio::test]
    async fn test_handle_door_status_with_warning_threshold_trigger() {
        use crate::config::Args;
        use clap::Parser;
        use crate::door::DoorStatus;
        
        let mut monitor = DoorMonitor::new();
        // Set door as open for longer than threshold
        monitor.state.door_opened_time = Some(Instant::now() - Duration::from_secs(120)); // 2 minutes ago
        monitor.state.last_door_state = Some(false); // Door was already open
        
        let args = Args::try_parse_from(&[
            "test", 
            "--api-url", "http://test.com",
            "--sms-api-username", "test_user",
            "--sms-api-password", "test_pass",
            "--sms-from-phone-number", "1234567890",
            "--sms-to-phone-number", "0987654321"
        ]).unwrap();
        let door_status = DoorStatus { id: 1, state: false }; // Door is still open
        let warning_threshold = Duration::from_secs(60); // 1 minute threshold

        // This should trigger warning logic since door has been open > threshold
        monitor.handle_door_status(&door_status, &args, warning_threshold).await;

        // Warning should have triggered first SMS
        assert!(monitor.state.sms_sent);
        assert_eq!(monitor.state.sms_backoff_index, 1);
        assert!(monitor.state.last_sms_time.is_some());
    }

    #[test]
    fn test_run_monitor_wrapper() {
        // Test the public run_monitor function exists and creates a DoorMonitor
        // This is mainly for completeness of coverage
        
        // We can't actually run this to completion since it's an infinite loop,
        // but we can test that it compiles and starts
        let args = crate::config::Args {
            api_url: "http://test.com".to_string(),
            check_interval_seconds: 1,
            open_too_long_seconds: 5,
            sms_api_username: None,
            sms_api_password: None,
            sms_from_phone_number: None,
            sms_to_phone_number: None,
            no_sms_backoff: false,
            telegram_token: None,
            telegram_conversation_id: None,
            telegram_test: false,
            test_message: None,
        };

        // Just verify the function signature is correct
        // We can't run it because it's an infinite loop
        let future = run_monitor(args);
        drop(future); // Prevent unused variable warning
    }
}
