// ============================================================================
// TEST UTILITIES - Common helpers for integration tests
// ============================================================================

use std::net::SocketAddr;
use axum::serve;
use tokio::net::TcpListener;
use drawing_app_backend::create_test_app;

// Spawn a test server and return its address
pub async fn spawn_test_server() -> SocketAddr {
    // Create the app
    let app = create_test_app().await;

    // Find a free port
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind to a port");

    let addr = listener.local_addr()
        .expect("Failed to get local address");

    // Start the server in the background
    tokio::spawn(async move {
        serve(listener, app)
            .await
            .expect("Failed to start test server");
    });

    // Give the server a moment to start
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    addr
}

// Create a test HTTP client
pub fn create_test_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .expect("Failed to create HTTP client")
}

// Helper to build test URLs
pub fn test_url(addr: SocketAddr, path: &str) -> String {
    format!("http://{}{}", addr, path)
}

// Helper to build test WebSocket URLs
pub fn test_ws_url(addr: SocketAddr, path: &str) -> String {
    format!("ws://{}{}", addr, path)
}