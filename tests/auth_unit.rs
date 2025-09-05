// ============================================================================
// AUTH UNIT TESTS - REORGANIZED
// Tests für einzelne Auth-Module
// ============================================================================

use drawing_app_backend::auth::*;
use chrono::Utc;

// ============================================================================
// JWT TESTS
// ============================================================================

#[tokio::test]
async fn test_jwt_creation_and_validation() {
    let test_email = "unittest@example.com";
    
    // JWT erstellen
    let token = create_jwt(test_email).unwrap();
    assert!(!token.is_empty());
    
    // JWT validieren
    let claims = validate_jwt(&token).unwrap();
    assert_eq!(claims.email, test_email);
    
    // Expiration prüfen (sollte ca. 24h in der Zukunft liegen)
    let now = Utc::now().timestamp() as usize;
    let exp_time = claims.exp;
    let time_diff = exp_time - now;
    
    assert!(time_diff > 82800); // > 23 Stunden
    assert!(time_diff < 90000); // < 25 Stunden
}

#[tokio::test]
async fn test_jwt_invalid_token() {
    let result = validate_jwt("invalid.jwt.token");
    assert!(result.is_err());
}

// ============================================================================
// PASSWORD TESTS
// ============================================================================

#[tokio::test]
async fn test_password_hashing() {
    let password = "mySecurePassword123!";
    
    let hash = hash_password(password).unwrap();
    assert!(!hash.is_empty());
    assert_ne!(hash, password);
    
    // Verifikation
    let is_valid = verify_password(password, &hash).unwrap();
    assert!(is_valid);
    
    let is_invalid = verify_password("wrongPassword", &hash).unwrap();
    assert!(!is_invalid);
}

#[tokio::test]
async fn test_password_hashing_uniqueness() {
    let password = "testPassword123";
    
    let hash1 = hash_password(password).unwrap();
    let hash2 = hash_password(password).unwrap();
    
    // Verschiedene Hashes wegen Salt
    assert_ne!(hash1, hash2);
    
    // Aber beide sollten das Passwort verifizieren
    assert!(verify_password(password, &hash1).unwrap());
    assert!(verify_password(password, &hash2).unwrap());
}

// ============================================================================
// USER STRUCT TESTS
// ============================================================================

#[tokio::test]
async fn test_user_creation() {
    let email = "newuser@example.com";
    let password = "userPassword123";
    
    let user = User::new(email.to_string(), "TestUser".to_string(), password).unwrap();
    
    assert_eq!(user.email, email);
    assert!(!user.id.is_empty());
    assert_ne!(user.password_hash, password);
    assert_eq!(user.id.len(), 36); // UUID-Format
}

#[tokio::test]
async fn test_user_password_verification() {
    let email = "testuser@example.com";
    let password = "myTestPassword";
    let wrong_password = "wrongPassword";
    
    let user = User::new(email.to_string(), "TestUser".to_string(), password).unwrap();
    
    assert!(user.verify_password(password).unwrap());
    assert!(!user.verify_password(wrong_password).unwrap());
}

#[tokio::test]
async fn test_unique_user_ids() {
    let password = "samePassword123";
    
    let user1 = User::new("user1@example.com".to_string(), "User1".to_string(), password).unwrap();
    let user2 = User::new("user2@example.com".to_string(), "User2".to_string(), password).unwrap();
    
    assert_ne!(user1.id, user2.id);
    assert_ne!(user1.password_hash, user2.password_hash);
}

// ============================================================================
// COOKIE TESTS
// ============================================================================

#[tokio::test]
async fn test_auth_cookie_creation() {
    let token = "sample.jwt.token";
    
    let cookie_header = create_auth_cookie(token);
    let cookie_string = cookie_header.to_str().unwrap();
    
    assert!(cookie_string.contains(&format!("auth_token={}", token)));
    assert!(cookie_string.contains("HttpOnly"));
    assert!(cookie_string.contains("Path=/"));
    assert!(cookie_string.contains("Max-Age=86400"));
    assert!(cookie_string.contains("SameSite=Strict"));
}

#[tokio::test]
async fn test_logout_cookie_creation() {
    let logout_cookie = create_logout_cookie();
    let cookie_string = logout_cookie.to_str().unwrap();
    
    assert!(cookie_string.contains("auth_token="));
    assert!(cookie_string.contains("Max-Age=0"));
    assert!(cookie_string.contains("HttpOnly"));
    assert!(cookie_string.contains("SameSite=Strict"));
}

// ============================================================================
// AUTH RESPONSE TESTS
// ============================================================================

#[tokio::test]
async fn test_auth_response_serialization() {
    let success_response = AuthResponse {
        success: true,
        message: "Login successful".to_string(),
        email: Some("test@example.com".to_string()),
    };
    
    let json_string = serde_json::to_string(&success_response).unwrap();
    assert!(json_string.contains("\"success\":true"));
    assert!(json_string.contains("\"message\":\"Login successful\""));
    assert!(json_string.contains("\"email\":\"test@example.com\""));
}

#[tokio::test]
async fn test_auth_response_deserialization() {
    let json_data = r#"{"success":true,"message":"Test message","email":"user@example.com"}"#;
    
    let response: AuthResponse = serde_json::from_str(json_data).unwrap();
    
    assert!(response.success);
    assert_eq!(response.message, "Test message");
    assert_eq!(response.email, Some("user@example.com".to_string()));
}