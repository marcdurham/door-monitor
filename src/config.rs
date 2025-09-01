use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Door sensor API URL
    #[arg(long)]
    pub api_url: Option<String>,

    /// Check interval in seconds
    #[arg(long, default_value = "5")]
    pub check_interval_seconds: u64,

    /// How many seconds is too long for the door to be open
    #[arg(long, default_value = "15")]
    pub open_too_long_seconds: u64,

    /// Disable SMS, ignores SMS arguments
    #[arg(long)]
    pub sms_off: bool,

    /// SMS API Username for voip.ms
    #[arg(long)]
    pub sms_api_username: Option<String>,

    /// SMS API Password for voip.ms  
    #[arg(long)]
    pub sms_api_password: Option<String>,

    /// SMS From Phone Number (DID)
    #[arg(long)]
    pub sms_from_phone_number: Option<String>,

    /// SMS To Phone Number
    #[arg(long)]
    pub sms_to_phone_number: Option<String>,

    /// Disable SMS backoff (send only one SMS instead of progressive intervals)
    #[arg(long)]
    pub no_sms_backoff: bool,

    /// Telegram Off, arguments ignored
    #[arg(long)]
    pub telegram_off: bool,

    /// Telegram Token
    #[arg(long)]
    pub telegram_token: Option<String>,

    /// Telegram Conversation ID
    #[arg(long)]
    pub telegram_conversation_id: Option<String>,

    /// Test Telegram
    #[arg(long)]
    pub telegram_test: bool,

    /// Test Message (Used for testing Telegram messages)
    #[arg(long)]
    pub test_message: Option<String>,
}

impl Args {
    pub fn sms_backoff(&self) -> bool {
        !self.no_sms_backoff
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_args_with_required_api_url() {
        let args = Args::try_parse_from(&[
            "door-monitor",
            "--api-url", "http://192.168.1.226/rpc/Input.GetStatus?id=0"
        ]).unwrap();

        assert_eq!(args.api_url, Some("http://192.168.1.226/rpc/Input.GetStatus?id=0"));
        assert_eq!(args.check_interval_seconds, 5); // default
        assert_eq!(args.open_too_long_seconds, 15); // default
        assert!(args.sms_backoff()); // default true
    }

    #[test]
    fn test_args_with_all_options() {
        let args = Args::try_parse_from(&[
            "door-monitor",
            "--api-url", "http://test.com",
            "--check-interval-seconds", "10",
            "--open-too-long-seconds", "30",
            "--sms-off",
            "--sms-api-username", "user123",
            "--sms-api-password", "pass456",
            "--sms-from-phone-number", "1234567890",
            "--sms-to-phone-number", "0987654321",
            "--no-sms-backoff",
            "--telegram-off",
            "--telegram-token", "2345:TEsttoKEN",
            "--telegram-conversation-id", "345678",
            "--telegram-test",
            "--test-message", "test message 1",
        ]).unwrap();

        assert_eq!(args.api_url, Some("http://test.com"));
        assert_eq!(args.check_interval_seconds, 10);
        assert_eq!(args.open_too_long_seconds, 30);
        assert!(args.sms_off);
        assert_eq!(args.sms_api_username, Some("user123".to_string()));
        assert_eq!(args.sms_api_password, Some("pass456".to_string()));
        assert_eq!(args.sms_from_phone_number, Some("1234567890".to_string()));
        assert_eq!(args.sms_to_phone_number, Some("0987654321".to_string()));
        assert_eq!(args.telegram_token, Some("2345:TEsttoKEN".to_string()));
        assert_eq!(args.telegram_conversation_id, Some("345678".to_string()));
        assert!(args.telegram_off);
        assert!(args.telegram_test);
        assert_eq!(args.test_message, Some("test message 1".to_string()));
        assert!(!args.sms_backoff());
    }

    #[test]
    fn test_args_missing_required_api_url() {
        let result = Args::try_parse_from(&["door-monitor"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_args_defaults() {
        let args = Args::try_parse_from(&[
            "door-monitor",
            "--api-url", "http://test.com"
        ]).unwrap();

        assert_eq!(args.check_interval_seconds, 5);
        assert_eq!(args.open_too_long_seconds, 15);
        assert!(args.sms_api_username.is_none());
        assert!(args.sms_api_password.is_none());
        assert!(args.sms_from_phone_number.is_none());
        assert!(args.sms_to_phone_number.is_none());
        assert!(args.sms_backoff());
    }

    #[test]
    fn test_args_custom_intervals() {
        let args = Args::try_parse_from(&[
            "door-monitor",
            "--api-url", "http://test.com",
            "--check-interval-seconds", "1",
            "--open-too-long-seconds", "60"
        ]).unwrap();

        assert_eq!(args.check_interval_seconds, 1);
        assert_eq!(args.open_too_long_seconds, 60);
    }

    #[test]
    fn test_args_sms_backoff_disabled() {
        let args = Args::try_parse_from(&[
            "door-monitor",
            "--api-url", "http://test.com",
            "--no-sms-backoff"
        ]).unwrap();

        assert!(!args.sms_backoff());
    }
}
