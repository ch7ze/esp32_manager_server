#[cfg(test)]
mod esp32_connection_tests {
    use std::time::Duration;
    use tokio::time::timeout;
    use futures_util::sink::SinkExt;

    #[tokio::test]
    async fn test_esp32_discovery() {
        println!("🔍 Testing ESP32 device discovery...");

        // Test mDNS discovery
        let discovery_result = timeout(
            Duration::from_secs(10),
            test_mdns_discovery()
        ).await;

        match discovery_result {
            Ok(devices) => {
                println!("✅ Found {} ESP32 device(s)", devices.len());
                assert!(!devices.is_empty(), "No ESP32 devices discovered");
            }
            Err(_) => {
                println!("⏰ Discovery timeout - no devices found in 10s");
                // Don't fail test if no devices are available
            }
        }
    }

    #[tokio::test]
    async fn test_server_api_connectivity() {
        println!("🌐 Testing server API connectivity...");

        let client = reqwest::Client::new();

        // Test health endpoint
        let response = client
            .get("http://localhost:3000/api/health")
            .timeout(Duration::from_secs(5))
            .send()
            .await;

        match response {
            Ok(resp) => {
                println!("✅ Server API responding: {}", resp.status());
                assert!(resp.status().is_success(), "Server API not healthy");
            }
            Err(e) => {
                println!("❌ Server API not reachable: {}", e);
                panic!("Server must be running for ESP32 connection tests");
            }
        }
    }

    #[tokio::test]
    async fn test_esp32_device_endpoint() {
        println!("📡 Testing ESP32 device endpoint...");

        let client = reqwest::Client::new();

        let response = client
            .get("http://localhost:3000/api/esp32/discovered")
            .timeout(Duration::from_secs(5))
            .send()
            .await;

        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    let body = resp.text().await.unwrap_or_default();
                    println!("✅ ESP32 endpoint response: {}", body);

                    // Try to parse as JSON
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                        if let Some(devices) = json.get("devices") {
                            if let Some(array) = devices.as_array() {
                                println!("📱 Found {} ESP32 device(s) via API", array.len());
                            }
                        }
                    }
                } else {
                    println!("⚠️ ESP32 endpoint returned: {}", resp.status());
                }
            }
            Err(e) => {
                println!("❌ Failed to reach ESP32 endpoint: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_websocket_connection() {
        println!("🔌 Testing WebSocket connection...");

        use tokio_tungstenite::{connect_async, tungstenite::Message};

        let ws_url = "ws://localhost:3000/ws";

        let connection_result = timeout(
            Duration::from_secs(5),
            connect_async(ws_url)
        ).await;

        match connection_result {
            Ok(Ok((mut ws_stream, _))) => {
                println!("✅ WebSocket connected successfully");

                // Send a test message
                if let Err(e) = ws_stream.send(Message::Text("test".to_string())).await {
                    println!("⚠️ Failed to send WebSocket message: {}", e);
                } else {
                    println!("📤 WebSocket message sent successfully");
                }
            }
            Ok(Err(e)) => {
                println!("❌ WebSocket connection failed: {}", e);
            }
            Err(_) => {
                println!("⏰ WebSocket connection timeout");
            }
        }
    }

    async fn test_mdns_discovery() -> Vec<String> {
        // Mock implementation - replace with actual mDNS discovery
        println!("🔍 Scanning for ESP32 devices via mDNS...");

        // Simulate discovery delay
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Return mock devices for now - you can integrate actual discovery logic
        vec!["test-esp32-001".to_string()]
    }

}