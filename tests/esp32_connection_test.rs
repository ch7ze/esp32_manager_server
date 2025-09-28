#[cfg(test)]
mod esp32_connection_tests {
    use std::time::Duration;
    use tokio::time::timeout;
    use futures_util::{sink::SinkExt, stream::StreamExt};
    use serde_json::Value;

    // Import test utilities
    mod common;
    use common::{spawn_test_server, create_test_client, test_url, test_ws_url};

    #[tokio::test]
    async fn test_server_basic_api() {
        println!("🌐 Testing server basic API...");

        // Spawn test server
        let addr = spawn_test_server().await;
        let client = create_test_client();

        // Test basic API endpoint
        let response = client
            .get(&test_url(addr, "/api"))
            .send()
            .await
            .expect("Failed to reach /api endpoint");

        assert!(response.status().is_success(), "API endpoint not healthy");

        let body = response.text().await.expect("Failed to read response body");
        println!("✅ Server API responding: {}", body);

        // Verify it's JSON and has expected structure
        let json: Value = serde_json::from_str(&body)
            .expect("API response is not valid JSON");

        assert!(json.get("title").is_some(), "Response missing 'title' field");
        assert!(json.get("status").is_some(), "Response missing 'status' field");
        println!("✅ API response has correct structure");
    }

    #[tokio::test]
    async fn test_esp32_discovered_devices_api() {
        println!("📡 Testing ESP32 discovered devices API...");

        // Spawn test server
        let addr = spawn_test_server().await;
        let client = create_test_client();

        let response = client
            .get(&test_url(addr, "/api/esp32/discovered"))
            .send()
            .await
            .expect("Failed to reach ESP32 discovered endpoint");

        assert!(response.status().is_success(), "ESP32 discovered endpoint failed");

        let body = response.text().await.expect("Failed to read response body");
        println!("✅ ESP32 endpoint response: {}", body);

        // Parse and validate JSON structure
        let json: Value = serde_json::from_str(&body)
            .expect("ESP32 API response is not valid JSON");

        // Verify expected structure
        assert!(json.get("devices").is_some(), "Response missing 'devices' field");
        assert!(json.get("count").is_some(), "Response missing 'count' field");

        if let Some(devices) = json["devices"].as_array() {
            println!("📱 Found {} ESP32 device(s) via API", devices.len());

            // Should have at least the test device we added
            assert!(devices.len() >= 1, "Should have at least one test device");

            // Validate each device has required fields
            for device in devices {
                assert!(device.get("name").is_some(), "Device missing 'name' field");
                assert!(device.get("ip").is_some(), "Device missing 'ip' field");
                assert!(device.get("tcp_port").is_some(), "Device missing 'tcp_port' field");
                assert!(device.get("udp_port").is_some(), "Device missing 'udp_port' field");
            }

            // Check for our test device
            let test_device = devices.iter().find(|d| {
                d.get("name").and_then(|n| n.as_str()) == Some("test-esp32-001")
            });
            assert!(test_device.is_some(), "Test device 'test-esp32-001' should be present");
        } else {
            panic!("Devices field is not an array");
        }
    }

    #[tokio::test]
    async fn test_websocket_connection_and_messaging() {
        println!("🔌 Testing WebSocket connection and messaging...");

        // Spawn test server
        let addr = spawn_test_server().await;

        use tokio_tungstenite::{connect_async, tungstenite::Message};

        let ws_url = test_ws_url(addr, "/channel");

        let connection_result = timeout(
            Duration::from_secs(5),
            connect_async(&ws_url)
        ).await
        .expect("WebSocket connection timeout")
        .expect("WebSocket connection failed");

        let (mut ws_stream, _) = connection_result;
        println!("✅ WebSocket connected successfully to {}", ws_url);

        // Send a test message
        let test_message = r#"{"type":"test","data":"integration_test"}"#;
        ws_stream.send(Message::Text(test_message.to_string())).await
            .expect("Failed to send WebSocket message");
        println!("📤 WebSocket message sent successfully");

        // Try to receive a response (with timeout)
        if let Ok(Some(response)) = timeout(Duration::from_secs(2), ws_stream.next()).await {
            match response {
                Ok(Message::Text(text)) => {
                    println!("📥 Received WebSocket response: {}", text);
                    // Verify it's valid JSON if it looks like JSON
                    if text.trim().starts_with('{') {
                        let _json: Value = serde_json::from_str(&text)
                            .expect("WebSocket response is not valid JSON");
                    }
                }
                Ok(msg) => {
                    println!("📥 Received WebSocket message: {:?}", msg);
                }
                Err(e) => {
                    println!("⚠️ WebSocket error: {}", e);
                }
            }
        } else {
            println!("⏰ No WebSocket response received (timeout - this is OK for basic test)");
        }

        // Close connection gracefully
        let _ = ws_stream.send(Message::Close(None)).await;
        println!("✅ WebSocket connection test completed");
    }

    #[tokio::test]
    async fn test_device_management_api() {
        println!("🔧 Testing device management API...");

        // Spawn test server
        let addr = spawn_test_server().await;
        let client = create_test_client();

        // Test GET /api/devices
        let response = client
            .get(&test_url(addr, "/api/devices"))
            .send()
            .await
            .expect("Failed to reach devices endpoint");

        assert!(response.status().is_success(), "Devices endpoint failed");

        let body = response.text().await.expect("Failed to read response body");
        println!("✅ Devices endpoint response: {}", body);

        // Parse and validate JSON
        let json: Value = serde_json::from_str(&body)
            .expect("Devices API response is not valid JSON");

        // Verify structure
        assert!(json.get("devices").is_some(), "Response missing 'devices' field");
        assert!(json.get("count").is_some(), "Response missing 'count' field");

        println!("📱 Device management API working correctly");
    }

}