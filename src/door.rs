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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_door_status_creation() {
        let status = DoorStatus { id: 0, state: true };
        assert_eq!(status.id, 0);
        assert_eq!(status.state, true);
    }

    #[test]
    fn test_door_status_closed() {
        let status = DoorStatus { id: 1, state: true };
        assert!(status.state); // Door is closed when state is true
    }

    #[test]
    fn test_door_status_open() {
        let status = DoorStatus { id: 0, state: false };
        assert!(!status.state); // Door is open when state is false
    }

    #[tokio::test]
    async fn test_check_door_status_success() {
        use mockito::Server;
        
        let mut server = Server::new_async().await;
        let mock = server.mock("GET", "/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id":0,"state":true}"#)
            .create_async()
            .await;

        let client = reqwest::Client::new();
        let result = check_door_status(&client, &server.url()).await;
        
        mock.assert_async().await;
        assert!(result.is_ok());
        
        let status = result.unwrap();
        assert_eq!(status.id, 0);
        assert_eq!(status.state, true);
    }

    #[tokio::test]
    async fn test_check_door_status_http_error() {
        use mockito::Server;
        
        let mut server = Server::new_async().await;
        let mock = server.mock("GET", "/")
            .with_status(500)
            .create_async()
            .await;

        let client = reqwest::Client::new();
        let result = check_door_status(&client, &server.url()).await;
        
        mock.assert_async().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("HTTP error: 500"));
    }

    #[tokio::test]
    async fn test_check_door_status_invalid_json() {
        use mockito::Server;
        
        let mut server = Server::new_async().await;
        let mock = server.mock("GET", "/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("invalid json")
            .create_async()
            .await;

        let client = reqwest::Client::new();
        let result = check_door_status(&client, &server.url()).await;
        
        mock.assert_async().await;
        assert!(result.is_err());
    }
}
