// ============================================================================
// AUTH INTEGRATION TESTS
// Umfassende Tests für das Authentifizierungs-System
// ============================================================================

use super::{setup_test_server, create_test_client, create_simple_client, json};
use crate::backend::fixtures::{TestUsers, TestEndpoints, ExpectedStatus, assertions};
use drawing_app_backend::AuthResponse;

// ============================================================================
// USER REGISTRATION TESTS
// ============================================================================

#[tokio::test]
async fn test_user_registration_success() {
    let (server_url, _user_store) = setup_test_server().await;
    let client = create_simple_client();
    
    let response = client
        .post(&TestEndpoints::register(&server_url))
        .header("Content-Type", "application/json")
        .json(&TestUsers::valid_user())
        .send()
        .await
        .unwrap();
    
    assertions::assert_successful_auth_response(response).await;
}

#[tokio::test]
async fn test_user_registration_duplicate_email() {
    let (server_url, _user_store) = setup_test_server().await;
    let client = create_simple_client();
    
    let user_data = TestUsers::valid_user();
    
    // Erste Registrierung - sollte erfolgreich sein
    let response1 = client
        .post(&TestEndpoints::register(&server_url))
        .header("Content-Type", "application/json")
        .json(&user_data)
        .send()
        .await
        .unwrap();
    
    assert_eq!(response1.status(), ExpectedStatus::SUCCESS);
    
    // Zweite Registrierung mit gleicher Email - sollte fehlschlagen
    let response2 = client
        .post(&TestEndpoints::register(&server_url))
        .header("Content-Type", "application/json")
        .json(&user_data)
        .send()
        .await
        .unwrap();
    
    assertions::assert_auth_error_response(&response2, ExpectedStatus::BAD_REQUEST);
    
    let auth_response: AuthResponse = response2.json().await.unwrap();
    assert!(!auth_response.success);
    assert_eq!(auth_response.message, "User already exists");
    assert_eq!(auth_response.email, None);
}

#[tokio::test]
async fn test_user_registration_sets_http_only_cookie() {
    let (server_url, _user_store) = setup_test_server().await;
    let client = create_test_client();
    
    let response = client
        .post(&TestEndpoints::register(&server_url))
        .header("Content-Type", "application/json")
        .json(&TestUsers::second_user())
        .send()
        .await
        .unwrap();
    
    assertions::assert_successful_auth_response(response).await;
}

// ============================================================================
// USER LOGIN TESTS  
// ============================================================================

#[tokio::test]
async fn test_user_login_success() {
    let (server_url, _user_store) = setup_test_server().await;
    let client = create_test_client();
    
    let user_data = TestUsers::valid_user();
    
    // Erst registrieren
    client
        .post(&TestEndpoints::register(&server_url))
        .header("Content-Type", "application/json")
        .json(&user_data)
        .send()
        .await
        .unwrap();
    
    // Dann einloggen
    let response = client
        .post(&TestEndpoints::login(&server_url))
        .header("Content-Type", "application/json")
        .json(&user_data)
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), ExpectedStatus::SUCCESS);
    
    let auth_response: AuthResponse = response.json().await.unwrap();
    assert!(auth_response.success);
    assert_eq!(auth_response.message, "Login successful");
    
    if let Some(ref email) = user_data["email"].as_str() {
        assert_eq!(auth_response.email, Some(email.to_string()));
    }
}

#[tokio::test]
async fn test_user_login_wrong_password() {
    let (server_url, _user_store) = setup_test_server().await;
    let client = create_simple_client();
    
    // Erst registrieren
    client
        .post(&TestEndpoints::register(&server_url))
        .header("Content-Type", "application/json")
        .json(&TestUsers::valid_user())
        .send()
        .await
        .unwrap();
    
    // Login mit falschem Passwort
    let response = client
        .post(&TestEndpoints::login(&server_url))
        .header("Content-Type", "application/json")
        .json(&TestUsers::user_with_wrong_password())
        .send()
        .await
        .unwrap();
    
    assertions::assert_auth_error_response(&response, ExpectedStatus::UNAUTHORIZED);
    
    let auth_response: AuthResponse = response.json().await.unwrap();
    assert!(!auth_response.success);
    assert_eq!(auth_response.message, "Invalid credentials");
    assert_eq!(auth_response.email, None);
}

#[tokio::test]
async fn test_user_login_nonexistent_user() {
    let (server_url, _user_store) = setup_test_server().await;
    let client = create_simple_client();
    
    // Login mit nicht existierendem User
    let response = client
        .post(&TestEndpoints::login(&server_url))
        .header("Content-Type", "application/json")
        .json(&TestUsers::nonexistent_user())
        .send()
        .await
        .unwrap();
    
    assertions::assert_auth_error_response(&response, ExpectedStatus::NOT_FOUND);
    
    let auth_response: AuthResponse = response.json().await.unwrap();
    assert!(!auth_response.success);
    assert_eq!(auth_response.message, "User not found");
    assert_eq!(auth_response.email, None);
}

// ============================================================================
// JWT TOKEN VALIDATION TESTS
// ============================================================================

#[tokio::test]
async fn test_token_validation_with_valid_cookie() {
    let (server_url, _user_store) = setup_test_server().await;
    let client = create_test_client();
    
    // Registrieren (setzt Cookie)
    client
        .post(&TestEndpoints::register(&server_url))
        .header("Content-Type", "application/json")
        .json(&TestUsers::valid_user())
        .send()
        .await
        .unwrap();
    
    // Token validieren mit Cookie
    let response = client
        .get(&TestEndpoints::validate_token(&server_url))
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), ExpectedStatus::SUCCESS);
}

#[tokio::test]
async fn test_token_validation_without_cookie() {
    let (server_url, _user_store) = setup_test_server().await;
    let client = create_simple_client(); // Kein Cookie Store
    
    // Token validieren ohne Cookie
    let response = client
        .get(&TestEndpoints::validate_token(&server_url))
        .send()
        .await
        .unwrap();
    
    assertions::assert_auth_error_response(&response, ExpectedStatus::UNAUTHORIZED);
}

// ============================================================================
// LOGOUT TESTS
// ============================================================================

#[tokio::test]
async fn test_user_logout_invalidates_token() {
    let (server_url, _user_store) = setup_test_server().await;
    let client = create_test_client();
    
    // Registrieren und einloggen
    client
        .post(&TestEndpoints::register(&server_url))
        .header("Content-Type", "application/json")
        .json(&TestUsers::valid_user())
        .send()
        .await
        .unwrap();
    
    // Logout
    let response = client
        .post(&TestEndpoints::logout(&server_url))
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), ExpectedStatus::SUCCESS);
    
    let auth_response: AuthResponse = response.json().await.unwrap();
    assert!(auth_response.success);
    assert_eq!(auth_response.message, "Logged out successfully");
    
    // Nach Logout sollte Token-Validation fehlschlagen
    let validate_response = client
        .get(&TestEndpoints::validate_token(&server_url))
        .send()
        .await
        .unwrap();
    
    assertions::assert_auth_error_response(&validate_response, ExpectedStatus::UNAUTHORIZED);
}

// ============================================================================
// COMPLETE AUTH FLOW TESTS
// ============================================================================

#[tokio::test]
async fn test_complete_authentication_flow() {
    let (server_url, _user_store) = setup_test_server().await;
    let client = create_test_client();
    
    let user_data = TestUsers::valid_user();
    
    // 1. Registrierung
    let register_response = client
        .post(&TestEndpoints::register(&server_url))
        .header("Content-Type", "application/json")
        .json(&user_data)
        .send()
        .await
        .unwrap();
    
    assert_eq!(register_response.status(), ExpectedStatus::SUCCESS);
    
    // 2. Token sollte nach Registrierung gültig sein
    let validate_response = client
        .get(&TestEndpoints::validate_token(&server_url))
        .send()
        .await
        .unwrap();
    
    assert_eq!(validate_response.status(), ExpectedStatus::SUCCESS);
    
    // 3. Logout
    let logout_response = client
        .post(&TestEndpoints::logout(&server_url))
        .send()
        .await
        .unwrap();
    
    assert_eq!(logout_response.status(), ExpectedStatus::SUCCESS);
    
    // 4. Nach Logout sollte Token ungültig sein
    let validate_after_logout = client
        .get(&TestEndpoints::validate_token(&server_url))
        .send()
        .await
        .unwrap();
    
    assert_eq!(validate_after_logout.status(), ExpectedStatus::UNAUTHORIZED);
    
    // 5. Login mit gleichen Credentials
    let login_response = client
        .post(&TestEndpoints::login(&server_url))
        .header("Content-Type", "application/json")
        .json(&user_data)
        .send()
        .await
        .unwrap();
    
    assert_eq!(login_response.status(), ExpectedStatus::SUCCESS);
    
    // 6. Token sollte nach Login wieder gültig sein
    let final_validate = client
        .get(&TestEndpoints::validate_token(&server_url))
        .send()
        .await
        .unwrap();
    
    assert_eq!(final_validate.status(), ExpectedStatus::SUCCESS);
}