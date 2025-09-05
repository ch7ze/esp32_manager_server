// ============================================================================
// AUTH INTEGRATION TESTS - REORGANIZED
// Umfassende Tests für das Authentifizierungs-System
// ============================================================================

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use reqwest::Client;
use serde_json::json;

use drawing_app_backend::{
    AuthResponse, UserStore, create_app
};

// ============================================================================
// TEST UTILITIES
// ============================================================================

/// Test-Server Setup
async fn setup_test_server() -> (String, UserStore) {
    let user_store: UserStore = Arc::new(RwLock::new(HashMap::new()));
    let app = create_app("test-hash".to_string(), user_store.clone()).await;
    
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server_url = format!("http://{}", addr);
    
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    (server_url, user_store)
}

/// Test-Client mit Cookie-Support
fn create_test_client() -> Client {
    Client::builder().cookie_store(true).build().unwrap()
}

/// Standard Test-User
fn valid_user() -> serde_json::Value {
    json!({"email": "test@example.com", "password": "testpass123"})
}

fn second_user() -> serde_json::Value {
    json!({"email": "user2@example.com", "password": "anotherpass456"})
}

// ============================================================================
// REGISTRATION TESTS
// ============================================================================

#[tokio::test]
async fn test_user_registration_success() {
    let (server_url, _) = setup_test_server().await;
    let client = Client::new();
    
    let response = client
        .post(&format!("{}/api/register", server_url))
        .header("Content-Type", "application/json")
        .json(&valid_user())
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), 200);
    
    let auth_response: AuthResponse = response.json().await.unwrap();
    assert!(auth_response.success);
    assert_eq!(auth_response.email, Some("test@example.com".to_string()));
}

#[tokio::test]
async fn test_user_registration_duplicate_email() {
    let (server_url, _) = setup_test_server().await;
    let client = Client::new();
    
    let user_data = valid_user();
    
    // Erste Registrierung
    client
        .post(&format!("{}/api/register", server_url))
        .header("Content-Type", "application/json")
        .json(&user_data)
        .send()
        .await
        .unwrap();
    
    // Zweite Registrierung - sollte fehlschlagen
    let response2 = client
        .post(&format!("{}/api/register", server_url))
        .header("Content-Type", "application/json")
        .json(&user_data)
        .send()
        .await
        .unwrap();
    
    assert_eq!(response2.status(), 400);
    
    let auth_response: AuthResponse = response2.json().await.unwrap();
    assert!(!auth_response.success);
    assert_eq!(auth_response.message, "User already exists");
}

#[tokio::test]
async fn test_registration_sets_http_only_cookie() {
    let (server_url, _) = setup_test_server().await;
    let client = create_test_client();
    
    let response = client
        .post(&format!("{}/api/register", server_url))
        .header("Content-Type", "application/json")
        .json(&second_user())
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), 200);
    
    let set_cookie_header = response.headers().get("set-cookie");
    assert!(set_cookie_header.is_some());
    
    let cookie_value = set_cookie_header.unwrap().to_str().unwrap();
    assert!(cookie_value.contains("auth_token="));
    assert!(cookie_value.contains("HttpOnly"));
    assert!(cookie_value.contains("SameSite=Strict"));
}

// ============================================================================
// LOGIN TESTS
// ============================================================================

#[tokio::test]
async fn test_user_login_success() {
    let (server_url, _) = setup_test_server().await;
    let client = create_test_client();
    
    let user_data = valid_user();
    
    // Registrieren
    client
        .post(&format!("{}/api/register", server_url))
        .header("Content-Type", "application/json")
        .json(&user_data)
        .send()
        .await
        .unwrap();
    
    // Login
    let response = client
        .post(&format!("{}/api/login", server_url))
        .header("Content-Type", "application/json")
        .json(&user_data)
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), 200);
    
    let auth_response: AuthResponse = response.json().await.unwrap();
    assert!(auth_response.success);
    assert_eq!(auth_response.message, "Login successful");
}

#[tokio::test]
async fn test_login_wrong_password() {
    let (server_url, _) = setup_test_server().await;
    let client = Client::new();
    
    // Registrieren
    client
        .post(&format!("{}/api/register", server_url))
        .header("Content-Type", "application/json")
        .json(&valid_user())
        .send()
        .await
        .unwrap();
    
    // Login mit falschem Passwort
    let wrong_pass = json!({"email": "test@example.com", "password": "wrongpass"});
    let response = client
        .post(&format!("{}/api/login", server_url))
        .header("Content-Type", "application/json")
        .json(&wrong_pass)
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), 401);
    
    let auth_response: AuthResponse = response.json().await.unwrap();
    assert!(!auth_response.success);
    assert_eq!(auth_response.message, "Invalid credentials");
}

// ============================================================================
// TOKEN VALIDATION TESTS
// ============================================================================

#[tokio::test]
async fn test_token_validation_with_cookie() {
    let (server_url, _) = setup_test_server().await;
    let client = create_test_client();
    
    // Registrieren (setzt Cookie)
    client
        .post(&format!("{}/api/register", server_url))
        .header("Content-Type", "application/json")
        .json(&valid_user())
        .send()
        .await
        .unwrap();
    
    // Token validieren
    let response = client
        .get(&format!("{}/api/validate-token", server_url))
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn test_token_validation_without_cookie() {
    let (server_url, _) = setup_test_server().await;
    let client = Client::new(); // Kein Cookie Store
    
    let response = client
        .get(&format!("{}/api/validate-token", server_url))
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), 401);
}

// ============================================================================
// LOGOUT TESTS
// ============================================================================

#[tokio::test]
async fn test_logout_invalidates_token() {
    let (server_url, _) = setup_test_server().await;
    let client = create_test_client();
    
    // Registrieren
    client
        .post(&format!("{}/api/register", server_url))
        .header("Content-Type", "application/json")
        .json(&valid_user())
        .send()
        .await
        .unwrap();
    
    // Logout
    let response = client
        .post(&format!("{}/api/logout", server_url))
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), 200);
    
    let auth_response: AuthResponse = response.json().await.unwrap();
    assert!(auth_response.success);
    assert_eq!(auth_response.message, "Logged out successfully");
    
    // Token sollte ungültig sein
    let validate_response = client
        .get(&format!("{}/api/validate-token", server_url))
        .send()
        .await
        .unwrap();
    
    assert_eq!(validate_response.status(), 401);
}

// ============================================================================
// COMPLETE FLOW TEST
// ============================================================================

#[tokio::test]
async fn test_complete_auth_flow() {
    let (server_url, _) = setup_test_server().await;
    let client = create_test_client();
    
    let user_data = valid_user();
    
    // 1. Register
    let register_response = client
        .post(&format!("{}/api/register", server_url))
        .header("Content-Type", "application/json")
        .json(&user_data)
        .send()
        .await
        .unwrap();
    assert_eq!(register_response.status(), 200);
    
    // 2. Validate token after registration
    let validate1 = client
        .get(&format!("{}/api/validate-token", server_url))
        .send()
        .await
        .unwrap();
    assert_eq!(validate1.status(), 200);
    
    // 3. Logout
    let logout_response = client
        .post(&format!("{}/api/logout", server_url))
        .send()
        .await
        .unwrap();
    assert_eq!(logout_response.status(), 200);
    
    // 4. Token should be invalid after logout
    let validate2 = client
        .get(&format!("{}/api/validate-token", server_url))
        .send()
        .await
        .unwrap();
    assert_eq!(validate2.status(), 401);
    
    // 5. Login again
    let login_response = client
        .post(&format!("{}/api/login", server_url))
        .header("Content-Type", "application/json")
        .json(&user_data)
        .send()
        .await
        .unwrap();
    assert_eq!(login_response.status(), 200);
    
    // 6. Token should be valid again
    let validate3 = client
        .get(&format!("{}/api/validate-token", server_url))
        .send()
        .await
        .unwrap();
    assert_eq!(validate3.status(), 200);
}