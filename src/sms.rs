use crate::config::Args;

pub async fn send_sms(
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
