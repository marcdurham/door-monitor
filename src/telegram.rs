use crate::config::Args;

pub async fn send_telegram(
    client: &reqwest::Client,
    args: &Args,
    message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Debug print all available arguments
    println!("Telegram Function Args Debug:");
    println!("  telegram_token {:?}", args.telegram_token.as_ref().map(|_| "[REDACTED]"));
    println!("  telegram_conversation_id: {:?}", args.telegram_conversation_id);
    println!("  message: {:?}", message);

    if let (Some(token), Some(conversation_id)) = (
        &args.telegram_token,
        &args.telegram_conversation_id,
    ) {
        let uri = format!("https://api.telegram.org/bot{}/sendMessage", token);

        println!("Telegram URI: {}", uri);
        println!("Telegram Conversation ID: {}", conversation_id.clone());
        println!("Test Message: {}", &message);

        let params = [
            ("chat_id", conversation_id.as_str()),
            ("text", message),
        ];

        let response = client
            .post(&uri)
            .form(&params)
            .send()
            .await?;

        println!("Telegram Response: {}", response.status().as_str());
        
        if response.status().is_success() {
            println!("Telegram sent successfully: {}", message);
        } else {
            eprintln!("Failed to send Telegram: HTTP {}", response.status());
        }
    } else {
        println!("Telegram args not supplied");
    }
    
    Ok(())
}
