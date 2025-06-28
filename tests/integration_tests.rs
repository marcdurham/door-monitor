use std::time::Duration;
use door_monitor::config::Args;
use door_monitor::door::{DoorStatus, check_door_status};
use door_monitor::utils::format_duration;
use clap::Parser;

#[tokio::test]
async fn test_door_monitor_integration() {
    use mockito::Server;
    
    let mut server = Server::new_async().await;
    let mock = server.mock("GET", "/")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"id":0,"state":true}"#)
        .create_async()
        .await;

    let args = Args::try_parse_from(&[
        "door-monitor",
        "--api-url", &server.url(),
        "--check-interval-seconds", "1",
        "--open-too-long-seconds", "5"
    ]).unwrap();

    let client = reqwest::Client::new();
    let result = check_door_status(&client, &args.api_url).await;
    
    mock.assert_async().await;
    assert!(result.is_ok());
    
    let status = result.unwrap();
    assert_eq!(status.id, 0);
    assert_eq!(status.state, true);
}

#[test]
fn test_duration_formatting_integration() {
    // Test various duration formats that might be used in the application
    let test_cases = vec![
        (0, "00:00:00"),
        (1, "00:00:01"),
        (60, "00:01:00"),
        (62, "00:01:02"),
        (3661, "01:01:01"),
        (86400, "1d 00:00:00"),
        (90061, "1d 01:01:01"),
        (90122, "1d 01:02:02"),
    ];

    for (seconds, expected) in test_cases {
        let duration = Duration::from_secs(seconds);
        assert_eq!(format_duration(duration), expected);
    }
}

#[test]
fn test_args_parsing_real_world_scenarios() {
    // Test realistic command line scenarios
    
    // Minimal setup
    let args = Args::try_parse_from(&[
        "door-monitor",
        "--api-url", "http://192.168.1.226/rpc/Input.GetStatus?id=0"
    ]).unwrap();
    assert_eq!(args.api_url, "http://192.168.1.226/rpc/Input.GetStatus?id=0");
    
    // Full SMS setup
    let args = Args::try_parse_from(&[
        "door-monitor",
        "--api-url", "http://192.168.1.226/rpc/Input.GetStatus?id=0",
        "--sms-api-username", "myuser",
        "--sms-api-password", "mypass",
        "--sms-from-phone-number", "5551234567",
        "--sms-to-phone-number", "5559876543",
        "--check-interval-seconds", "10",
        "--open-too-long-seconds", "30"
    ]).unwrap();
    
    assert!(args.sms_api_username.is_some());
    assert!(args.sms_api_password.is_some());
    assert!(args.sms_from_phone_number.is_some());
    assert!(args.sms_to_phone_number.is_some());
    assert_eq!(args.check_interval_seconds, 10);
    assert_eq!(args.open_too_long_seconds, 30);
}

#[tokio::test]
async fn test_error_handling_scenarios() {
    use mockito::Server;
    
    let mut server = Server::new_async().await;
    
    // Test 404 error
    let mock_404 = server.mock("GET", "/notfound")
        .with_status(404)
        .create_async()
        .await;

    let client = reqwest::Client::new();
    let result = check_door_status(&client, &format!("{}/notfound", server.url())).await;
    
    mock_404.assert_async().await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("404"));
}

#[test]
fn test_door_status_serialization() {
    // Test that DoorStatus can be properly serialized/deserialized
    let status = DoorStatus { id: 42, state: false };
    let json = serde_json::to_string(&status).unwrap();
    assert_eq!(json, r#"{"id":42,"state":false}"#);
    
    let deserialized: DoorStatus = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.id, 42);
    assert_eq!(deserialized.state, false);
}
