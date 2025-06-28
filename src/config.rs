use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Door sensor API URL
    #[arg(long)]
    pub api_url: String,

    /// Check interval in seconds
    #[arg(long, default_value = "5")]
    pub check_interval: u64,

    /// Warning threshold in seconds
    #[arg(long, default_value = "15")]
    pub warning_threshold: u64,

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

    /// Enable SMS backoff (5,15,30,60 minutes then every 60 minutes)
    #[arg(long, default_value = "true")]
    pub sms_backoff: bool,
}
