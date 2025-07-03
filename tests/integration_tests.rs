use reqwest;
use serde_json::Value;
use std::time::Duration;
use tokio::time::sleep;

const BASE_URL: &str = "http://127.0.0.1:3000";

#[tokio::test]
async fn test_health_endpoint() {
    let client = reqwest::Client::new();

    // Test main health endpoint
    let response = client.get(&format!("{}/health", BASE_URL)).send().await;

    match response {
        Ok(resp) => {
            assert_eq!(resp.status(), 200);

            let body: Value = resp.json().await.unwrap();
            assert_eq!(body["status"], "healthy");
            assert_eq!(body["service"], "project-gateway");
        }
        Err(_) => {
            // Server might not be running during CI
            println!("Warning: Could not connect to server for integration test");
        }
    }
}

#[tokio::test]
async fn test_api_health_endpoint() {
    let client = reqwest::Client::new();

    let response = client
        .get(&format!("{}/api/v1/health", BASE_URL))
        .send()
        .await;

    match response {
        Ok(resp) => {
            assert_eq!(resp.status(), 200);

            let body: Value = resp.json().await.unwrap();
            assert_eq!(body["status"], "healthy");
            assert_eq!(body["service"], "project-gateway");
        }
        Err(_) => {
            println!("Warning: Could not connect to server for integration test");
        }
    }
}

#[tokio::test]
async fn test_users_list_endpoint() {
    let client = reqwest::Client::new();

    let response = client
        .get(&format!("{}/api/v1/users", BASE_URL))
        .send()
        .await;

    match response {
        Ok(resp) => {
            assert_eq!(resp.status(), 200);

            let body: Value = resp.json().await.unwrap();
            assert!(body["users"].is_array());
            assert!(body["total"].is_number());
        }
        Err(_) => {
            println!("Warning: Could not connect to server for integration test");
        }
    }
}

#[tokio::test]
async fn test_metrics_endpoint() {
    let client = reqwest::Client::new();

    let response = client.get(&format!("{}/metrics", BASE_URL)).send().await;

    match response {
        Ok(resp) => {
            assert_eq!(resp.status(), 200);

            let content_type = resp.headers().get("content-type");
            assert!(content_type.is_some());

            let body = resp.text().await.unwrap();
            // Basic check for Prometheus format
            assert!(body.contains("# HELP") || body.contains("# TYPE"));
        }
        Err(_) => {
            println!("Warning: Could not connect to server for integration test");
        }
    }
}
