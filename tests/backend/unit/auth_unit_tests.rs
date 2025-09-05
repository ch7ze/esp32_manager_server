// ============================================================================
// AUTH UNIT TESTS
// Tests für einzelne Auth-Module ohne HTTP-Server
// ============================================================================

use super::*;
use chrono::{Duration, Utc};

// ============================================================================
// JWT TOKEN TESTS
// ============================================================================

#[tokio::test]
async fn test_jwt_creation_and_validation() {
    let test_email = "unittest@example.com";
    
    // JWT erstellen
    let user = User::new(test_email.to_string(), "TestUser".to_string(), "password123").unwrap();
    let token = create_jwt(&user).unwrap();
    assert!(!token.is_empty(), "JWT token should not be empty");
    
    // JWT validieren
    let claims = validate_jwt(&token).unwrap();
    assert_eq!(claims.email, test_email);
    
    // Prüfen ob Expiration korrekt ist (sollte ca. 24h in der Zukunft liegen)
    let now = Utc::now().timestamp() as usize;
    let exp_time = claims.exp;
    let time_diff = exp_time - now;
    
    // Sollte zwischen 23 und 25 Stunden liegen (24h ± 1h Toleranz)
    assert!(time_diff > 82800, "Token should expire in more than 23 hours"); // 23 * 3600
    assert!(time_diff < 90000, "Token should expire in less than 25 hours"); // 25 * 3600
}

#[tokio::test]
async fn test_jwt_with_invalid_token() {
    let invalid_token = "invalid.jwt.token";
    
    let result = validate_jwt(invalid_token);
    assert!(result.is_err(), "Invalid JWT should fail validation");
}

#[tokio::test]
async fn test_jwt_with_empty_token() {
    let empty_token = "";
    
    let result = validate_jwt(empty_token);
    assert!(result.is_err(), "Empty JWT should fail validation");
}

// ============================================================================
// PASSWORD HASHING TESTS
// ============================================================================

#[tokio::test]
async fn test_password_hashing_and_verification() {
    let test_password = "mySecurePassword123!";
    
    // Passwort hashen
    let hash = hash_password(test_password).unwrap();
    assert!(!hash.is_empty(), "Password hash should not be empty");
    assert_ne!(hash, test_password, "Hash should be different from original password");
    
    // Passwort verifizieren
    let is_valid = verify_password(test_password, &hash).unwrap();
    assert!(is_valid, "Password verification should succeed");
    
    // Falsches Passwort sollte fehlschlagen
    let wrong_password = "wrongPassword";
    let is_invalid = verify_password(wrong_password, &hash).unwrap();
    assert!(!is_invalid, "Wrong password should fail verification");
}

#[tokio::test]
async fn test_password_hashing_consistency() {
    let test_password = "testPassword123";
    
    // Gleiches Passwort mehrfach hashen sollte unterschiedliche Hashes ergeben
    // (wegen Salt in bcrypt)
    let hash1 = hash_password(test_password).unwrap();
    let hash2 = hash_password(test_password).unwrap();
    
    assert_ne!(hash1, hash2, "Different hashes should be generated due to salt");
    
    // Aber beide sollten das ursprüngliche Passwort verifizieren
    assert!(verify_password(test_password, &hash1).unwrap());
    assert!(verify_password(test_password, &hash2).unwrap());
}

// ============================================================================
// USER STRUCT TESTS
// ============================================================================

#[tokio::test]
async fn test_user_creation() {
    let test_email = "newuser@example.com";
    let test_password = "userPassword123";
    
    // User erstellen
    let user = User::new(test_email.to_string(), "TestUser".to_string(), test_password).unwrap();
    
    // Validierung der User-Properties
    assert_eq!(user.email, test_email);
    assert!(!user.id.is_empty(), "User ID should not be empty");
    assert_ne!(user.password_hash, test_password, "Password should be hashed");
    
    // UUID-Format prüfen (grob)
    assert_eq!(user.id.len(), 36, "UUID should be 36 characters long");
    assert!(user.id.contains('-'), "UUID should contain hyphens");
}

#[tokio::test]
async fn test_user_password_verification() {
    let test_email = "testuser@example.com";
    let test_password = "myTestPassword";
    let wrong_password = "wrongPassword";
    
    let user = User::new(test_email.to_string(), "TestUser".to_string(), test_password).unwrap();
    
    // Korrektes Passwort sollte funktionieren
    let correct_verification = user.verify_password(test_password).unwrap();
    assert!(correct_verification, "Correct password should be verified");
    
    // Falsches Passwort sollte fehlschlagen
    let wrong_verification = user.verify_password(wrong_password).unwrap();
    assert!(!wrong_verification, "Wrong password should fail verification");
}

#[tokio::test]
async fn test_multiple_users_have_unique_ids() {
    let email1 = "user1@example.com";
    let email2 = "user2@example.com";
    let password = "samePassword123";
    
    let user1 = User::new(email1.to_string(), "User1".to_string(), password).unwrap();
    let user2 = User::new(email2.to_string(), "User2".to_string(), password).unwrap();
    
    // IDs sollten unterschiedlich sein
    assert_ne!(user1.id, user2.id, "Different users should have different IDs");
    
    // Aber beide sollten das gleiche Passwort haben (unterschiedlich gehashed)
    assert!(user1.verify_password(password).unwrap());
    assert!(user2.verify_password(password).unwrap());
    assert_ne!(user1.password_hash, user2.password_hash, "Same password should hash differently");
}

// ============================================================================
// COOKIE HELPER TESTS
// ============================================================================

#[tokio::test]
async fn test_auth_cookie_creation() {
    let test_token = "sample.jwt.token";
    
    let cookie_header = create_auth_cookie(test_token);
    let cookie_string = cookie_header.to_str().unwrap();
    
    // Cookie-Attribute prüfen
    assert!(cookie_string.contains(&format!("auth_token={}", test_token)), 
           "Cookie should contain the token");
    assert!(cookie_string.contains("HttpOnly"), 
           "Cookie should be HttpOnly");
    assert!(cookie_string.contains("Path=/"), 
           "Cookie should have Path=/");
    assert!(cookie_string.contains("Max-Age=86400"), 
           "Cookie should have 24h expiration");
    assert!(cookie_string.contains("SameSite=Strict"), 
           "Cookie should have SameSite=Strict");
}

#[tokio::test]
async fn test_logout_cookie_creation() {
    let logout_cookie = create_logout_cookie();
    let cookie_string = logout_cookie.to_str().unwrap();
    
    // Logout-Cookie-Attribute prüfen
    assert!(cookie_string.contains("auth_token="), 
           "Logout cookie should clear auth_token");
    assert!(cookie_string.contains("Max-Age=0"), 
           "Logout cookie should have immediate expiration");
    assert!(cookie_string.contains("HttpOnly"), 
           "Logout cookie should be HttpOnly");
    assert!(cookie_string.contains("SameSite=Strict"), 
           "Logout cookie should have SameSite=Strict");
}

// ============================================================================
// AUTH RESPONSE TESTS
// ============================================================================

#[tokio::test]
async fn test_auth_response_serialization() {
    // Erfolgreiche Response
    let success_response = AuthResponse {
        success: true,
        message: "Login successful".to_string(),
        email: Some("test@example.com".to_string()),
    };
    
    let json_string = serde_json::to_string(&success_response).unwrap();
    assert!(json_string.contains("\"success\":true"));
    assert!(json_string.contains("\"message\":\"Login successful\""));
    assert!(json_string.contains("\"email\":\"test@example.com\""));
    
    // Fehler-Response
    let error_response = AuthResponse {
        success: false,
        message: "Invalid credentials".to_string(),
        email: None,
    };
    
    let error_json = serde_json::to_string(&error_response).unwrap();
    assert!(error_json.contains("\"success\":false"));
    assert!(error_json.contains("\"message\":\"Invalid credentials\""));
    assert!(error_json.contains("\"email\":null"));
}

#[tokio::test]
async fn test_auth_response_deserialization() {
    let json_data = r#"{"success":true,"message":"Test message","email":"user@example.com"}"#;
    
    let response: AuthResponse = serde_json::from_str(json_data).unwrap();
    
    assert!(response.success);
    assert_eq!(response.message, "Test message");
    assert_eq!(response.email, Some("user@example.com".to_string()));
}