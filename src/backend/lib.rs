// ============================================================================
// LIB.RS - LIBRARY EXPORTS FOR TESTING
// Makes internal modules available as library for integration tests
// ============================================================================

use std::sync::Arc;
use axum::{Router, Json, extract::State, http::StatusCode};
use serde_json::{json, Value};

// All modules
pub mod app_state;
pub mod auth;
pub mod file_utils;
pub mod database;
pub mod device_store;
pub mod events;
pub mod websocket;
pub mod device_types;
pub mod device_connection;
pub mod device_manager;
pub mod device_discovery;
pub mod mdns_discovery;
pub mod mdns_server;
pub mod debug_logger;
pub mod uart_connection;

// Re-export key types for tests
pub use app_state::AppState;
pub use database::DatabaseManager;
pub use device_store::{create_shared_store, SharedDeviceStore};

// Create a test-friendly app instance
pub async fn create_test_app() -> Router {
    // Initialize minimal components for testing
    let db = Arc::new(DatabaseManager::new().await.expect("Failed to create test database"));
    let device_store = create_shared_store();
    let device_manager = device_manager::create_device_manager(device_store.clone());

    // Start device manager for tests
    device_manager.start().await;

    let device_discovery = Arc::new(tokio::sync::Mutex::new(
        device_discovery::DeviceDiscovery::with_manager(device_store.clone(), Some(device_manager.clone()), None)
    ));

    let mdns_server = Arc::new(tokio::sync::Mutex::new(
        mdns_server::MdnsServer::new().expect("Failed to create test mDNS server")
    ));

    // Create UART connection for tests with empty shared state trackers
    use std::collections::HashMap;
    use tokio::sync::RwLock;
    let unified_connection_states = Arc::new(RwLock::new(HashMap::new()));
    let unified_activity_tracker = Arc::new(RwLock::new(HashMap::new()));
    let device_connection_types = Arc::new(RwLock::new(HashMap::new()));

    let uart_connection = Arc::new(tokio::sync::Mutex::new(
        uart_connection::UartConnection::new(
            device_store.clone(),
            unified_connection_states,
            unified_activity_tracker,
            device_connection_types,
        )
    ));

    // Add test device for consistent testing
    let ip = std::net::IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 43, 75));
    let test_device = device_types::DeviceConfig::new(
        "test-device-001".to_string(),
        ip,
        3232,
        3232,
    );
    let _ = device_manager.add_device(test_device).await;

    // Create app using the internal function
    create_app_internal(db, device_store, device_manager, device_discovery, mdns_server, uart_connection).await
}

// Internal helper to create app for testing
// This avoids circular imports with main.rs
async fn create_app_internal(
    db: Arc<DatabaseManager>,
    device_store: SharedDeviceStore,
    device_manager: Arc<device_manager::DeviceManager>,
    device_discovery: Arc<tokio::sync::Mutex<device_discovery::DeviceDiscovery>>,
    mdns_server: Arc<tokio::sync::Mutex<mdns_server::MdnsServer>>,
    uart_connection: Arc<tokio::sync::Mutex<uart_connection::UartConnection>>,
) -> Router {
    use axum::routing::get;
    use tower::ServiceBuilder;
    use tower_http::trace::TraceLayer;

    let mut app = Router::new();

    // AppState for all handlers
    let app_state = AppState {
        db: db.clone(),
        device_store: device_store.clone(),
        device_manager: device_manager.clone(),
        device_discovery: device_discovery.clone(),
        mdns_server: mdns_server.clone(),
        uart_connection: uart_connection.clone(),
    };

    // API Routes
    let api_routes = Router::new()
        .route("/api", get(api_home))
        .route("/api/users", get(api_users))
        .route("/api/devices/discovered", get(discovered_devices_handler))
        .route("/api/devices", get(list_devices_handler))
        .with_state(app_state.clone());

    // WebSocket routes
    let websocket_routes = Router::new()
        .route("/channel", get(websocket_handler))
        .with_state(app_state.clone());

    // Merge routes
    app = app.merge(api_routes);
    app = app.merge(websocket_routes);

    // Add middleware
    app = app.layer(
        ServiceBuilder::new()
            .layer(TraceLayer::new_for_http())
    );

    app
}

// Handler functions
async fn api_home() -> Json<Value> {
    Json(json!({
        "title": "Device Manager Backend",
        "status": "running",
        "version": "0.1.0"
    }))
}

async fn api_users() -> Json<Value> {
    Json(json!({ "users": [] }))
}

async fn discovered_devices_handler(
    State(app_state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    // Get discovered devices from DeviceDiscovery service
    let discovered_devices = {
        let discovery = app_state.device_discovery.lock().await;
        discovery.get_discovered_devices().await
    };

    // Convert to JSON format expected by tests
    let devices: Vec<Value> = discovered_devices
        .into_iter()
        .map(|(name, device)| {
            json!({
                "name": name,
                "ip": device.device_config.ip_address.to_string(),
                "tcp_port": device.device_config.tcp_port,
                "udp_port": device.udp_port
            })
        })
        .collect();

    Ok(Json(json!({
        "devices": devices,
        "count": devices.len()
    })))
}

async fn list_devices_handler(
    State(app_state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    // Get devices from device store
    let devices = app_state.device_store.get_active_devices().await;

    Ok(Json(json!({
        "devices": devices,
        "count": devices.len()
    })))
}

async fn websocket_handler() -> &'static str {
    "WebSocket endpoint - use proper WebSocket client to connect"
}