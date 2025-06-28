use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct DoorStatus {
    pub id: u8,
    pub state: bool,
}

pub async fn check_door_status(
    client: &reqwest::Client, 
    api_url: &str
) -> Result<DoorStatus, Box<dyn std::error::Error>> {
    let response = client.get(api_url).send().await?;
    
    if response.status().is_success() {
        let door_status: DoorStatus = response.json().await?;
        Ok(door_status)
    } else {
        Err(format!("HTTP error: {}", response.status()).into())
    }
}
