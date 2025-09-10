// Authentication module for user management and ESP32 device management

use axum::http::HeaderValue;
use bcrypt::{hash, verify, DEFAULT_COST};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use uuid::Uuid;

// JWT secret key - should be loaded from environment variable in production
const JWT_SECRET: &[u8] = b"your-secret-key-should-be-much-longer-and-random";

// Data structures for authentication

// ESP32 Device representation with permissions and status  
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ESP32Device {
    pub id: String,
    pub name: String,
    pub mac_address: String,
    pub ip_address: Option<String>,
    pub status: String,
    pub maintenance_mode: bool,
    pub owner_id: String,
    pub firmware_version: Option<String>,
    pub last_seen: String,
    pub created_at: String,
    pub permissions: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateDeviceRequest {
    pub name: String,
    pub mac_address: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateDeviceRequest {
    pub name: Option<String>,
    pub maintenance_mode: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePermissionRequest {
    pub user_id: String,
    pub permission: String,
}

// Registered user representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub display_name: String,
    pub password_hash: String,
}

// JWT token claims
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub user_id: String,
    pub email: String,
    pub display_name: String,
    pub device_permissions: HashMap<String, String>,
    pub exp: usize,
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub display_name: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateDisplayNameRequest {
    pub display_name: String,
}

// Response structure for authentication APIs
#[derive(Debug, Serialize, Deserialize)]
pub struct AuthResponse {
    pub success: bool,
    pub message: String,
    pub email: Option<String>,
}

// Thread-safe user store
pub type UserStore = Arc<RwLock<HashMap<String, User>>>;

// Thread-safe ESP32 device store
pub type DeviceStore = Arc<RwLock<HashMap<String, ESP32Device>>>;

// JWT token creation and validation
pub fn create_jwt(user: &User) -> Result<String, jsonwebtoken::errors::Error> {
    // Token expires after 24 hours
    let expiration = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(24))
        .expect("valid timestamp")
        .timestamp() as usize;

    // Sample device permissions for demo purposes
    let mut device_permissions = HashMap::new();
    device_permissions.insert("esp32-abc123-def456-ghi789".to_string(), "R".to_string());
    device_permissions.insert("esp32-jkl012-mno345-pqr678".to_string(), "W".to_string());
    device_permissions.insert("esp32-stu901-vwx234-yza567".to_string(), "V".to_string());
    device_permissions.insert("esp32-bcd890-efg123-hij456".to_string(), "M".to_string());
    device_permissions.insert("esp32-klm789-nop012-qrs345".to_string(), "O".to_string());

    // Token claims
    let claims = Claims {
        user_id: user.id.clone(),
        email: user.email.clone(),
        display_name: user.display_name.clone(),
        device_permissions,
        exp: expiration,
    };

    // Create and sign the token
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(JWT_SECRET),
    )
}

// Create JWT with actual device permissions from store
pub fn create_jwt_with_device_permissions(user: &User, device_store: &HashMap<String, ESP32Device>) -> Result<String, jsonwebtoken::errors::Error> {
    // Token expires after 24 hours
    let expiration = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(24))
        .expect("valid timestamp")
        .timestamp() as usize;

    // Load actual device permissions from store
    let device_permissions = get_user_device_permissions(device_store, &user.id);

    // Token claims
    let claims = Claims {
        user_id: user.id.clone(),
        email: user.email.clone(),
        display_name: user.display_name.clone(),
        device_permissions,
        exp: expiration,
    };

    // Create and sign the token
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(JWT_SECRET),
    )
}

// Validates a JWT token and returns the claims
// Website feature: Checks if a user is still logged in
pub fn validate_jwt(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    // Decrypt token and verify signature
    decode::<Claims>(
        token,                                    // JWT string
        &DecodingKey::from_secret(JWT_SECRET),   // Verification with secret
        &Validation::default(),                  // Standard validation (expiration date etc.)
    )
    .map(|data| data.claims)  // Only return claims, not the whole token
}

// ============================================================================
// PASSWORD SECURITY - Bcrypt hashing against brute-force attacks
// Website feature: Secure password storage
// ============================================================================

// Hashes a password with Bcrypt (slow and secure)
// Website feature: Called during registration
pub fn hash_password(password: &str) -> Result<String, bcrypt::BcryptError> {
    // DEFAULT_COST = 12 rounds (2^12 = 4096 hashing iterations)
    // Higher cost = more secure but slower
    hash(password, DEFAULT_COST)
}

// Checks if a password matches the hash
// Website feature: Called during login
pub fn verify_password(password: &str, hash: &str) -> Result<bool, bcrypt::BcryptError> {
    // Bcrypt hashes the password with the same salt and compares
    verify(password, hash)
}

// ============================================================================
// USER IMPLEMENTATION - Methods for user objects
// Website feature: User creation and password verification
// ============================================================================

// impl block defines methods for the User struct
impl User {
    // Creates a new user with hashed password
    // Website feature: Called during registration
    pub fn new(email: String, display_name: String, password: &str) -> Result<Self, bcrypt::BcryptError> {
        // Hash password (? = propagates error upwards)
        let password_hash = hash_password(password)?;
        
        Ok(User {
            id: Uuid::new_v4().to_string(), // Generate random UUID (immutable)
            email,
            display_name,
            password_hash,
        })
    }

    // Checks if the given password is correct
    // Website feature: Called during login
    pub fn verify_password(&self, password: &str) -> Result<bool, bcrypt::BcryptError> {
        // Delegiert an die globale verify_password Funktion
        verify_password(password, &self.password_hash)
    }

    // Aktualisiert den Anzeigenamen des Users
    // Website-Feature: Wird für Profil-Updates verwendet
    pub fn update_display_name(&mut self, new_display_name: String) {
        self.display_name = new_display_name;
    }
}


// ============================================================================
// COOKIE HELPER - Erstellt sichere HTTP-Cookies
// Website-Feature: Login-State im Browser speichern
// ============================================================================

// Erstellt ein sicheres Auth-Cookie mit JWT Token
// Website-Feature: Wird nach erfolgreichem Login gesetzt
pub fn create_auth_cookie(token: &str) -> HeaderValue {
    let cookie_value = format!(
        "auth_token={}; HttpOnly; Path=/; Max-Age=86400; SameSite=Strict",
        token
    );
    // HttpOnly = JavaScript kann nicht auf Cookie zugreifen (XSS-Schutz)
    // Path=/ = Cookie gilt für ganze Website
    // Max-Age=86400 = Cookie läuft nach 24h ab (86400 Sekunden)
    // SameSite=Strict = Schutz vor CSRF-Attacken
    HeaderValue::from_str(&cookie_value).unwrap()
}

// Erstellt ein Logout-Cookie (löscht das Auth-Cookie)
// Website-Feature: Wird beim Logout aufgerufen
pub fn create_logout_cookie() -> HeaderValue {
    // Max-Age=0 = Cookie sofort löschen
    let cookie_value = "auth_token=; HttpOnly; Path=/; Max-Age=0; SameSite=Strict";
    HeaderValue::from_str(cookie_value).unwrap()
}

// ============================================================================
// ESP32 DEVICE MANAGEMENT - Funktionen für ESP32-Verwaltung und Berechtigungen
// Website-Feature: A 5.4 Rechtesystem Implementation adapted for ESP32
// ============================================================================

impl ESP32Device {
    // Erstellt ein neues ESP32 Device mit dem Owner als einzigem Benutzer
    pub fn new(name: String, mac_address: String, owner_id: String) -> Self {
        let device_id = format!("esp32-{}", uuid::Uuid::new_v4());
        let mut permissions = HashMap::new();
        permissions.insert(owner_id.clone(), "O".to_string()); // Owner-Berechtigung
        
        ESP32Device {
            id: device_id,
            name,
            mac_address,
            ip_address: None,
            status: "Offline".to_string(),
            maintenance_mode: false,
            owner_id,
            firmware_version: None,
            last_seen: chrono::Utc::now().to_rfc3339(),
            created_at: chrono::Utc::now().to_rfc3339(),
            permissions,
        }
    }

    // Prüft ob ein User eine bestimmte Berechtigung für dieses ESP32 Device hat
    pub fn has_permission(&self, user_id: &str, required_permission: &str) -> bool {
        match self.permissions.get(user_id) {
            Some(user_permission) => {
                // Hierarchie der Berechtigungen prüfen
                match required_permission {
                    "R" => ["R", "W", "V", "M", "O"].contains(&user_permission.as_str()),
                    "W" => {
                        if self.maintenance_mode {
                            // Im Wartungsmodus können nur V, M und O Befehle senden (W nicht)
                            ["V", "M", "O"].contains(&user_permission.as_str())
                        } else {
                            // Normal braucht man mindestens W-Berechtigung
                            ["W", "V", "M", "O"].contains(&user_permission.as_str())
                        }
                    },
                    "V" => ["V", "M", "O"].contains(&user_permission.as_str()),
                    "M" => ["M", "O"].contains(&user_permission.as_str()),
                    "O" => user_permission == "O",
                    _ => false,
                }
            }
            None => false,
        }
    }

    // Prüft ob ein User Berechtigungen vergeben darf
    pub fn can_grant_permission(&self, user_id: &str, permission_to_grant: &str) -> bool {
        match self.permissions.get(user_id) {
            Some(user_permission) => {
                match user_permission.as_str() {
                    "O" => true, // Owner kann alle Berechtigungen vergeben
                    "M" => ["R", "W", "V"].contains(&permission_to_grant), // Moderator kann bis V vergeben
                    _ => false,
                }
            }
            None => false,
        }
    }

    // Aktualisiert die Berechtigung eines Users
    pub fn update_permission(&mut self, user_id: String, permission: String) {
        if permission == "REMOVE" {
            self.permissions.remove(&user_id);
        } else {
            self.permissions.insert(user_id, permission);
        }
    }

    // Aktualisiert den Status des ESP32 Devices
    pub fn update_status(&mut self, status: String, ip_address: Option<String>) {
        self.status = status;
        self.ip_address = ip_address;
        self.last_seen = chrono::Utc::now().to_rfc3339();
    }
}

// Hilfsfunktion: Erstellt Device-Berechtigungen für JWT basierend auf User-ID
pub fn get_user_device_permissions(device_store: &HashMap<String, ESP32Device>, user_id: &str) -> HashMap<String, String> {
    let mut permissions = HashMap::new();
    
    for (device_id, device) in device_store {
        if let Some(permission) = device.permissions.get(user_id) {
            permissions.insert(device_id.clone(), permission.clone());
        }
    }
    
    permissions
}

// Validiert eine Berechtigung (R, W, V, M, O)
pub fn is_valid_permission(permission: &str) -> bool {
    ["R", "W", "V", "M", "O"].contains(&permission)
}

