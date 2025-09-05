// ============================================================================
// LIB.RS - BIBLIOTHEKS-EXPORTE FÜR TESTS
// Macht interne Module für Integration Tests verfügbar
// ============================================================================

pub mod auth;
pub mod file_utils;

// Re-export wichtiger Typen und Funktionen für Tests
pub use crate::auth::{User, UserStore, AuthResponse, LoginRequest, RegisterRequest, UpdateDisplayNameRequest};
use axum::{
    body::Body,
    extract::State,
    http::{StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{get, post, Router},
    Json,
};
use axum_extra::extract::CookieJar;
use serde_json::{json, Value};
use tower::ServiceBuilder;
use tower_http::{
    services::ServeDir,
    trace::TraceLayer,
};
use mime_guess;

use auth::{
    create_auth_cookie,
    create_jwt,
    create_logout_cookie,
    validate_jwt,
};

use file_utils::{handle_spa_route, handle_spa_route_with_cache_control};

// ============================================================================
// CREATE APP FUNCTION - FÜR TESTS EXPORTIERT
// ============================================================================

pub async fn create_app(client_hash: String, user_store: UserStore) -> Router {
    let mut app = Router::new();

    // API ROUTES
    let api_routes = Router::new()
        .route("/api", get(api_home))
        .route("/api/users", get(api_users))
        .route("/api/register", post(register_handler))
        .route("/api/login", post(login_handler))
        .route("/api/logout", post(logout_handler))
        .route("/api/validate-token", get(validate_token_handler))
        .route("/api/user-info", get(user_info_handler))
        .route("/api/profile/display-name", post(update_display_name_handler))
        .with_state(user_store);

    app = app.merge(api_routes);

    // Serve static files from 'public' directory (stylesheets)
    app = app.nest_service("/stylesheets", ServeDir::new("public/stylesheets"));

    // SPA routes with explicit handlers
    app = app
        .route("/index.html", get(serve_spa_route))
        .route("/login.html", get(serve_spa_route))
        .route("/login", get(serve_spa_route))
        .route("/register.html", get(serve_spa_route))
        .route("/register", get(serve_spa_route))
        .route("/debug.html", get(serve_spa_route))
        .route("/hallo.html", get(serve_spa_route))  
        .route("/about.html", get(serve_spa_route))
        .route("/drawing_board.html", get(serve_spa_route))
        .route("/drawer_page.html", get(serve_spa_route));

    // Handle hashed SPA routes (with long-term caching)
    if !client_hash.is_empty() {
        app = app
            .route(&format!("/{}/index.html", client_hash), get(serve_hashed_spa_route))
            .route(&format!("/{}/login.html", client_hash), get(serve_hashed_spa_route))
            .route(&format!("/{}/login", client_hash), get(serve_hashed_spa_route))
            .route(&format!("/{}/register.html", client_hash), get(serve_hashed_spa_route))
            .route(&format!("/{}/register", client_hash), get(serve_hashed_spa_route))
            .route(&format!("/{}/debug.html", client_hash), get(serve_hashed_spa_route))
            .route(&format!("/{}/hallo.html", client_hash), get(serve_hashed_spa_route))
            .route(&format!("/{}/about.html", client_hash), get(serve_hashed_spa_route))
            .route(&format!("/{}/drawing_board.html", client_hash), get(serve_hashed_spa_route))
            .route(&format!("/{}/drawer_page.html", client_hash), get(serve_hashed_spa_route));
    }

    // Static files are handled by explicit SPA routes above
    // No ServeDir needed for templates, scripts, styles

    // Serve static files with hash in URL path (1-year cache)
    if !client_hash.is_empty() {
        let hashed_path = format!("/{}", client_hash);
        app = app.nest_service(&hashed_path, ServeDir::new("dest"));
    }

    // Root path serves SPA
    app = app.route("/", get(serve_spa_route));

    // Serve remaining static files from dest
    app = app
        .route("/index.css", get(|| serve_static_file("dest/index.css")))
        .route("/app.js", get(|| serve_static_file("dest/app.js")));

    // Catch-all for SPA fallback
    app = app.fallback(spa_fallback);

    // Add middleware
    app = app.layer(
        ServiceBuilder::new()
            .layer(TraceLayer::new_for_http())
    );

    app
}

// ============================================================================
// HANDLER FUNCTIONS - KOPIERT AUS MAIN.RS FÜR TESTS
// ============================================================================

async fn api_home() -> Json<Value> {
    Json(json!({ "title": "Express" }))
}

async fn api_users() -> Json<Value> {
    Json(json!({ "users": [] }))
}

async fn serve_spa_route() -> Response<Body> {
    handle_spa_route().await
}

async fn serve_hashed_spa_route() -> Response<Body> {
    handle_spa_route_with_cache_control("public, max-age=31536000, immutable").await
}

async fn serve_static_file(file_path: &str) -> impl IntoResponse {
    match std::fs::read(file_path) {
        Ok(contents) => {
            let mime_type = mime_guess::from_path(file_path)
                .first_or_octet_stream()
                .to_string();
            
            Response::builder()
                .header("content-type", mime_type)
                .body(Body::from(contents))
                .unwrap()
        }
        Err(_) => {
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("File not found"))
                .unwrap()
        }
    }
}

async fn spa_fallback(uri: Uri) -> impl IntoResponse {
    let path = uri.path();
    
    if path.starts_with("/api") || path.contains('.') {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body("Not Found".into())
            .unwrap();
    }
    
    handle_spa_route().await
}

// AUTHENTICATION HANDLERS
async fn register_handler(
    State(user_store): State<UserStore>,
    Json(req): Json<RegisterRequest>,
) -> Result<Response<Body>, StatusCode> {
    
    tracing::info!("Registration attempt for email: {}", req.email);
    tracing::debug!("Register request received: {:?}", req.email);
    
    {
        let users = user_store.read().await;
        if users.contains_key(&req.email) {
            tracing::warn!("Registration failed: User {} already exists", req.email);
            let response = AuthResponse {
                success: false,
                message: "User already exists".to_string(),
                email: None,
            };
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&response).unwrap()))
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    tracing::debug!("Creating new user with hashed password");
    match User::new(req.email.clone(), req.display_name.clone(), &req.password) {
        Ok(user) => {
            tracing::debug!("🎫 Creating JWT token for new user");
            match create_jwt(&user) {
                Ok(token) => {
                    // User in Datenbank speichern NACH erfolgreichem JWT
                    {
                        let mut users = user_store.write().await;
                        users.insert(req.email.clone(), user);
                        tracing::debug!("User {} stored in database", req.email);
                    }
                    
                    tracing::info!("Registration successful for user: {}", req.email);
                    let response = AuthResponse {
                        success: true,
                        message: "User registered successfully".to_string(),
                        email: Some(req.email.clone()),
                    };

                    Response::builder()
                        .header("set-cookie", create_auth_cookie(&token))
                        .header("content-type", "application/json")
                        .body(Body::from(serde_json::to_string(&response).unwrap()))
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
                }
                Err(e) => {
                    tracing::error!("JWT creation failed for {}: {:?}", req.email, e);
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                },
            }
        }
        Err(e) => {
            tracing::error!("User creation failed for {}: {:?}", req.email, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        },
    }
}

async fn login_handler(
    State(user_store): State<UserStore>,
    Json(req): Json<LoginRequest>,
) -> Result<Response<Body>, StatusCode> {
    
    tracing::info!("🔑 Login attempt for email: {}", req.email);
    tracing::debug!("Login request received for: {}", req.email);
    
    let user = {
        let users = user_store.read().await;
        users.get(&req.email).cloned()
    };

    match user {
        Some(user) => {
            tracing::debug!("User found in database: {}", req.email);
            match user.verify_password(&req.password) {
                Ok(true) => {
                    tracing::debug!("Password verification successful");
                    match create_jwt(&user) {
                        Ok(token) => {
                            tracing::info!("Login successful for user: {}", req.email);
                            let response = AuthResponse {
                                success: true,
                                message: "Login successful".to_string(),
                                email: Some(req.email.clone()),
                            };

                            Response::builder()
                                .header("set-cookie", create_auth_cookie(&token))
                                .header("content-type", "application/json")
                                .body(Body::from(serde_json::to_string(&response).unwrap()))
                                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
                        }
                        Err(e) => {
                            tracing::error!("JWT creation failed during login for {}: {:?}", req.email, e);
                            Err(StatusCode::INTERNAL_SERVER_ERROR)
                        },
                    }
                }
                Ok(false) => {
                    tracing::warn!("🚫 Login failed: Invalid password for {}", req.email);
                    let response = AuthResponse {
                        success: false,
                        message: "Invalid credentials".to_string(),
                        email: None,
                    };
                    Response::builder()
                        .status(StatusCode::UNAUTHORIZED)
                        .header("content-type", "application/json")
                        .body(Body::from(serde_json::to_string(&response).unwrap()))
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
                }
                Err(e) => {
                    tracing::error!("Password verification error for {}: {:?}", req.email, e);
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                },
            }
        }
        None => {
            tracing::warn!("🚫 Login failed: User {} not found", req.email);
            let response = AuthResponse {
                success: false,
                message: "User not found".to_string(),
                email: None,
            };
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&response).unwrap()))
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn logout_handler() -> Response<Body> {
    let response = AuthResponse {
        success: true,
        message: "Logged out successfully".to_string(),
        email: None,
    };

    Response::builder()
        .header("set-cookie", create_logout_cookie())
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&response).unwrap()))
        .unwrap()
}

async fn validate_token_handler(cookie_jar: CookieJar) -> StatusCode {
    let token = match cookie_jar.get("auth_token") {
        Some(cookie) => cookie.value(),
        None => return StatusCode::UNAUTHORIZED,
    };

    match validate_jwt(token) {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::UNAUTHORIZED,
    }
}

async fn user_info_handler(cookie_jar: CookieJar) -> Result<Json<Value>, StatusCode> {
    let token = match cookie_jar.get("auth_token") {
        Some(cookie) => cookie.value(),
        None => return Err(StatusCode::UNAUTHORIZED),
    };

    let claims = match validate_jwt(token) {
        Ok(claims) => claims,
        Err(_) => return Err(StatusCode::UNAUTHORIZED),
    };

    Ok(Json(json!({
        "success": true,
        "user_id": claims.user_id,
        "email": claims.email,
        "display_name": claims.display_name
    })))
}

async fn update_display_name_handler(
    State(user_store): State<UserStore>,
    cookie_jar: CookieJar,
    Json(req): Json<UpdateDisplayNameRequest>,
) -> Result<Response<Body>, StatusCode> {
    let token = match cookie_jar.get("auth_token") {
        Some(cookie) => cookie.value(),
        None => return Err(StatusCode::UNAUTHORIZED),
    };

    let claims = match validate_jwt(token) {
        Ok(claims) => claims,
        Err(_) => return Err(StatusCode::UNAUTHORIZED),
    };

    if req.display_name.trim().is_empty() || req.display_name.len() > 50 {
        let response = AuthResponse {
            success: false,
            message: "Display name must be between 1 and 50 characters".to_string(),
            email: None,
        };
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&response).unwrap()))
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);
    }

    {
        let mut users = user_store.write().await;
        if let Some(user) = users.get_mut(&claims.email) {
            user.update_display_name(req.display_name.trim().to_string());
        } else {
            return Err(StatusCode::NOT_FOUND);
        }
    }

    let updated_user = {
        let users = user_store.read().await;
        users.get(&claims.email).cloned()
    };

    match updated_user {
        Some(user) => {
            match create_jwt(&user) {
                Ok(new_token) => {
                    let response = AuthResponse {
                        success: true,
                        message: "Display name updated successfully".to_string(),
                        email: Some(claims.email),
                    };

                    Response::builder()
                        .header("set-cookie", create_auth_cookie(&new_token))
                        .header("content-type", "application/json")
                        .body(Body::from(serde_json::to_string(&response).unwrap()))
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
                }
                Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

// ============================================================================
// UNIT TESTS - Für VS Code Test Explorer
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_auth_types() {
        let login_req = LoginRequest {
            email: "test@example.com".to_string(),
            password: "password123".to_string(),
        };
        
        assert_eq!(login_req.email, "test@example.com");
        assert_eq!(login_req.password, "password123");
    }

    #[test]
    fn test_register_request_creation() {
        let register_req = RegisterRequest {
            email: "test@example.com".to_string(),
            display_name: "Test User".to_string(),
            password: "password123".to_string(),
        };
        
        assert_eq!(register_req.email, "test@example.com");
        assert_eq!(register_req.display_name, "Test User");
        assert_eq!(register_req.password, "password123");
    }

    #[test]
    fn test_auth_response_success() {
        let response = AuthResponse {
            success: true,
            message: "Login successful".to_string(),
            email: Some("test@example.com".to_string()),
        };
        
        assert!(response.success);
        assert_eq!(response.message, "Login successful");
        assert_eq!(response.email, Some("test@example.com".to_string()));
    }

    #[test]
    fn test_auth_response_failure() {
        let response = AuthResponse {
            success: false,
            message: "Invalid credentials".to_string(),
            email: None,
        };
        
        assert!(!response.success);
        assert_eq!(response.message, "Invalid credentials");
        assert_eq!(response.email, None);
    }
}