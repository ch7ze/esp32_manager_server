// ============================================================================
// INTEGRATION TESTS MODULE
// Gemeinsame Utilities für Backend Integration Tests
// ============================================================================

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use drawing_app_backend::{UserStore, create_app};

/// Test-Server Setup - Startet einen lokalen Server für Integration Tests
pub async fn setup_test_server() -> (String, UserStore) {
    let user_store: UserStore = Arc::new(RwLock::new(HashMap::new()));
    let app = create_app("test-hash".to_string(), user_store.clone()).await;
    
    // Startet den Server auf einem freien Port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server_url = format!("http://{}", addr);
    
    // Server im Hintergrund starten
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    
    // Kurz warten bis Server bereit ist
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    (server_url, user_store)
}

/// Test-Client mit Cookie-Support erstellen
pub fn create_test_client() -> reqwest::Client {
    reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .unwrap()
}

/// Test-Client ohne Cookie-Support erstellen
pub fn create_simple_client() -> reqwest::Client {
    reqwest::Client::new()
}

// Re-export wichtiger Test-Dependencies
pub use reqwest::Client;
pub use serde_json::json;

// Module exports
pub mod auth_tests;