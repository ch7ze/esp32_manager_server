// ============================================================================
// TEST FIXTURES MODULE
// Test-Daten und Helper-Funktionen für wiederverwendbare Tests
// ============================================================================

use serde_json::{json, Value};

/// Standard Test-User für Tests
pub struct TestUsers;

impl TestUsers {
    /// Gültiger Test-User für erfolgreiche Tests
    pub fn valid_user() -> Value {
        json!({
            "email": "test@example.com",
            "password": "testpass123"
        })
    }
    
    /// Zweiter gültiger Test-User für Konflikt-Tests
    pub fn second_user() -> Value {
        json!({
            "email": "user2@example.com", 
            "password": "anotherpass456"
        })
    }
    
    /// User mit ungültigem Passwort
    pub fn user_with_wrong_password() -> Value {
        json!({
            "email": "test@example.com",
            "password": "wrongpassword"
        })
    }
    
    /// Nicht existierender User
    pub fn nonexistent_user() -> Value {
        json!({
            "email": "nonexistent@example.com",
            "password": "anypassword"
        })
    }
}

/// HTTP-Endpunkte für Tests
pub struct TestEndpoints;

impl TestEndpoints {
    pub fn register(base_url: &str) -> String {
        format!("{}/api/register", base_url)
    }
    
    pub fn login(base_url: &str) -> String {
        format!("{}/api/login", base_url)
    }
    
    pub fn logout(base_url: &str) -> String {
        format!("{}/api/logout", base_url)
    }
    
    pub fn validate_token(base_url: &str) -> String {
        format!("{}/api/validate-token", base_url)
    }
}

/// Erwartete HTTP-Status-Codes
pub struct ExpectedStatus;

impl ExpectedStatus {
    pub const SUCCESS: u16 = 200;
    pub const BAD_REQUEST: u16 = 400;
    pub const UNAUTHORIZED: u16 = 401;
    pub const NOT_FOUND: u16 = 404;
    pub const INTERNAL_SERVER_ERROR: u16 = 500;
}

/// Test-Assertions Helper
pub mod assertions {
    use reqwest::Response;
    
    /// Prüft ob Response erfolgreich ist und Cookie gesetzt wurde
    pub async fn assert_successful_auth_response(response: Response) {
        assert_eq!(response.status(), 200);
        
        // Prüfen ob Set-Cookie Header vorhanden ist
        let set_cookie_header = response.headers().get("set-cookie");
        assert!(set_cookie_header.is_some(), "Auth cookie should be set");
        
        let cookie_value = set_cookie_header.unwrap().to_str().unwrap();
        assert!(cookie_value.contains("auth_token="), "Should contain auth_token");
        assert!(cookie_value.contains("HttpOnly"), "Should be HttpOnly");
        assert!(cookie_value.contains("SameSite=Strict"), "Should be SameSite=Strict");
    }
    
    /// Prüft ob Response ein Auth-Fehler ist
    pub fn assert_auth_error_response(response: &Response, expected_status: u16) {
        assert_eq!(response.status(), expected_status);
    }
}