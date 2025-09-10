// ============================================================================
// WEBSOCKET HANDLER - WebSocket Communication for ESP32 Device Management
// ============================================================================

use crate::auth::{validate_jwt, Claims};
use crate::device_store::{SharedDeviceStore};
use crate::events::{ClientMessage, ServerMessage, DeviceEvent};
use crate::database::DatabaseManager;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State, ConnectInfo,
    },
    response::Response,
    http::StatusCode,
};
use axum_extra::extract::CookieJar;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc;
use futures::{sink::SinkExt, stream::StreamExt};
use tracing::{info, warn, error, debug};
use serde_json;

// ============================================================================
// APPLICATION STATE FOR WEBSOCKET
// ============================================================================

#[derive(Clone)]
pub struct WebSocketState {
    pub device_store: SharedDeviceStore,
    pub db: Arc<DatabaseManager>,
    pub esp32_manager: Arc<crate::esp32_manager::Esp32Manager>,
}

// ============================================================================
// WEBSOCKET UPGRADE HANDLER
// ============================================================================

/// WebSocket upgrade handler with JWT authentication
/// Route: GET /channel/
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<WebSocketState>,
    cookie_jar: CookieJar,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Result<Response, (StatusCode, String)> {
    info!("🔥 WebSocket handler called from {}", addr);
    
    // Check if this is a proper WebSocket upgrade request
    info!("Headers: Connection upgrade request");
    
    // JWT Token authentication for WebSocket
    let token = match cookie_jar.get("auth_token") {
        Some(cookie) => cookie.value(),
        None => {
            warn!("WebSocket: No auth token found");
            return Err((StatusCode::UNAUTHORIZED, "Authentication required".to_string()));
        }
    };
    
    // Validate JWT token
    let claims = match crate::auth::validate_jwt(token) {
        Ok(claims) => {
            info!("WebSocket authenticated user: {} ({})", claims.display_name, claims.email);
            claims
        },
        Err(e) => {
            warn!("WebSocket: Invalid JWT token: {:?}", e);
            return Err((StatusCode::UNAUTHORIZED, "Invalid authentication token".to_string()));
        }
    };
    
    // Generate unique client ID for this connection
    let client_id = generate_client_id(&claims.email);
    
    // Upgrade to WebSocket connection
    let response = ws.on_upgrade(move |socket| {
        handle_websocket_connection(socket, state, claims, client_id, addr)
    });
    
    Ok(response)
}

// ============================================================================
// WEBSOCKET CONNECTION HANDLING
// ============================================================================

/// Handle an individual WebSocket connection
async fn handle_websocket_connection(
    socket: WebSocket,
    state: WebSocketState,
    jwt_claims: Claims,
    client_id: String,
    addr: SocketAddr,
) {
    info!("WebSocket connection established for client {} (user: {}, addr: {})", 
          client_id, jwt_claims.email, addr);
    
    let user_id = jwt_claims.user_id.clone();
    let display_name = jwt_claims.display_name.clone();
    let (mut sender, mut receiver) = socket.split();
    
    // Create channel for sending messages to this client
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMessage>();
    
    // Clone client_id for the outgoing task
    let client_id_for_task = client_id.clone();
    
    // Spawn task to handle outgoing messages
    let outgoing_task = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            match serde_json::to_string(&message) {
                Ok(json) => {
                    if let Err(e) = sender.send(Message::Text(json)).await {
                        error!("Failed to send WebSocket message: {}", e);
                        break;
                    }
                }
                Err(e) => {
                    error!("Failed to serialize message: {}", e);
                }
            }
        }
        debug!("Outgoing message task ended for client {}", client_id_for_task);
    });
    
    // Handle incoming messages
    let device_store = state.device_store.clone();
    let db = state.db.clone();
    let mut registered_devices: Vec<String> = Vec::new();
    
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                match handle_client_message(
                    &text, 
                    &device_store, 
                    &db,
                    &state.esp32_manager,
                    &user_id,
                    &display_name,
                    &client_id, 
                    &tx,
                    &mut registered_devices
                ).await {
                    Ok(()) => {
                        debug!("Processed message from client {}: {}", client_id, text);
                    }
                    Err(e) => {
                        error!("Error processing message from client {}: {}", client_id, e);
                        // Send error response back to client
                        let error_response = ServerMessage::device_events(
                            "error".to_string(),
                            vec![]
                        );
                        if let Err(send_err) = tx.send(error_response) {
                            error!("Failed to send error response: {}", send_err);
                        }
                    }
                }
            }
            Ok(Message::Close(_)) => {
                info!("WebSocket connection closed by client {}", client_id);
                break;
            }
            Ok(Message::Ping(_data)) => {
                debug!("Received ping from client {}", client_id);
                // Pong will be sent automatically by axum
            }
            Ok(Message::Pong(_)) => {
                debug!("Received pong from client {}", client_id);
            }
            Ok(Message::Binary(_)) => {
                warn!("Received unexpected binary message from client {}", client_id);
            }
            Err(e) => {
                error!("WebSocket error for client {}: {}", client_id, e);
                break;
            }
        }
    }
    
    // Cleanup: unregister from all devices
    for device_id in registered_devices {
        if let Err(e) = device_store.unregister_client(&device_id, &client_id).await {
            error!("Failed to unregister client {} from device {}: {}", client_id, device_id, e);
        }
    }
    
    // Cancel outgoing task
    outgoing_task.abort();
    
    info!("WebSocket connection terminated for client {} (user: {})", client_id, user_id);
}

// ============================================================================
// MESSAGE HANDLING
// ============================================================================

/// Handle incoming client message
async fn handle_client_message(
    message_text: &str,
    device_store: &SharedDeviceStore,
    db: &Arc<DatabaseManager>,
    esp32_manager: &Arc<crate::esp32_manager::Esp32Manager>,
    user_id: &str,
    display_name: &str,
    client_id: &str,
    tx: &mpsc::UnboundedSender<ServerMessage>,
    registered_devices: &mut Vec<String>,
) -> Result<(), String> {
    // First, try to parse as a generic JSON to check for heartbeat messages
    if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(message_text) {
        if let Some(msg_type) = json_value.get("type").and_then(|t| t.as_str()) {
            if msg_type == "ping" {
                // Handle heartbeat ping - send pong response
                debug!("Received ping from client {}, sending pong", client_id);
                
                // Extract timestamp from ping message if present
                let timestamp = json_value.get("timestamp")
                    .and_then(|t| t.as_u64());
                
                // Send pong response using existing message channel
                let pong_response = ServerMessage::pong(timestamp);
                tx.send(pong_response)
                    .map_err(|e| format!("Failed to send pong response: {}", e))?;
                
                debug!("Sent pong response to client {}", client_id);
                return Ok(());
            }
        }
    }
    
    // Parse as ClientMessage for actual canvas operations
    debug!("Parsing ClientMessage JSON: {}", message_text);
    let client_message: ClientMessage = serde_json::from_str(message_text)
        .map_err(|e| {
            error!("Failed to parse ClientMessage JSON: {}", e);
            error!("Raw JSON: {}", message_text);
            format!("Invalid ClientMessage JSON: {}", e)
        })?;
    
    match client_message {
        ClientMessage::RegisterForDevice { device_id } => {
            handle_register_for_device(
                device_id,
                device_store,
                db,
                user_id,
                display_name,
                client_id,
                tx,
                registered_devices
            ).await
        }
        
        ClientMessage::UnregisterForDevice { device_id } => {
            handle_unregister_for_device(
                device_id,
                device_store,
                client_id,
                registered_devices
            ).await
        }
        
        ClientMessage::DeviceEvent { device_id, events_for_device } => {
            handle_device_events(
                device_id,
                events_for_device,
                device_store,
                db,
                esp32_manager,
                user_id,
                client_id,
                registered_devices
            ).await
        }
    }
}

/// Handle registerForDevice command
async fn handle_register_for_device(
    device_id: String,
    device_store: &SharedDeviceStore,
    db: &Arc<DatabaseManager>,
    user_id: &str,
    display_name: &str,
    client_id: &str,
    tx: &mpsc::UnboundedSender<ServerMessage>,
    registered_devices: &mut Vec<String>,
) -> Result<(), String> {
    // Check if user has permission to access this device (requires at least Read permission)
    let has_permission = db.user_has_device_permission(&device_id, user_id, "R").await
        .map_err(|e| format!("Database error checking permissions: {}", e))?;
    
    if !has_permission {
        return Err(format!("User {} does not have permission to access device {}", user_id, device_id));
    }
    
    info!("User {} has access permission for device {}", user_id, device_id);
    
    info!("Registering client {} for device {} (user: {})", client_id, device_id, user_id);
    
    // Register client and get existing events for replay
    let existing_events = device_store.register_client(
        device_id.clone(),
        user_id.to_string(),
        display_name.to_string(),
        client_id.to_string(),
        tx.clone()
    ).await?;
    
    // Add to registered devices list
    if !registered_devices.contains(&device_id) {
        registered_devices.push(device_id.clone());
    }
    
    // Send existing events to client for replay
    if !existing_events.is_empty() {
        let event_count = existing_events.len();
        let response = ServerMessage::device_events(
            device_id.clone(),
            existing_events
        );
        
        tx.send(response)
            .map_err(|e| format!("Failed to send events to client: {}", e))?;
        
        info!("Sent {} existing events to client {} for device {}", 
              event_count, client_id, device_id);
    } else {
        debug!("No existing events to send to client {} for device {}", client_id, device_id);
    }
    
    Ok(())
}

/// Handle unregisterForDevice command
async fn handle_unregister_for_device(
    device_id: String,
    device_store: &SharedDeviceStore,
    client_id: &str,
    registered_devices: &mut Vec<String>,
) -> Result<(), String> {
    info!("Unregistering client {} from device {}", client_id, device_id);
    
    // Unregister from device store
    device_store.unregister_client(&device_id, client_id).await?;
    
    // Remove from registered devices list
    registered_devices.retain(|id| id != &device_id);
    
    Ok(())
}

/// Handle device events from client
async fn handle_device_events(
    device_id: String,
    events: Vec<DeviceEvent>,
    device_store: &SharedDeviceStore,
    db: &Arc<DatabaseManager>,
    esp32_manager: &Arc<crate::esp32_manager::Esp32Manager>,
    user_id: &str,
    client_id: &str,
    registered_devices: &[String],
) -> Result<(), String> {
    // Check if client is registered for this device
    if !registered_devices.contains(&device_id) {
        return Err(format!("Client {} is not registered for device {}", client_id, device_id));
    }
    
    // Check write permissions for device operations
    let has_write_permission = db.user_has_device_permission(&device_id, user_id, "W").await
        .map_err(|e| format!("Database error checking write permissions: {}", e))?;
    
    if !has_write_permission {
        return Err(format!("User {} does not have write permission for device {}", user_id, device_id));
    }
    
    info!("User {} has write permission for device {}", user_id, device_id);
    
    // Process each event
    for event in events {
        debug!("Processing event from client {} for device {}: {:?}", client_id, device_id, event);
        
        // Check if this is an ESP32 command event
        if let DeviceEvent::Esp32Command { command, .. } = &event {
            // Handle ESP32 command via ESP32 manager
            if let Err(e) = esp32_manager.handle_websocket_command(
                &device_id,
                command.clone(),
                user_id,
                client_id,
            ).await {
                error!("Failed to handle ESP32 command for device {}: {}", device_id, e);
                return Err(format!("ESP32 command failed: {}", e));
            }
            
            debug!("ESP32 command processed successfully for device {}", device_id);
            continue; // ESP32 manager handles the event broadcasting
        }
        
        // Add event to store (this will also broadcast to other clients)
        device_store.add_event(device_id.clone(), event, user_id.to_string(), client_id.to_string()).await?;
    }
    
    Ok(())
}

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

/// Extract and validate JWT from HTTP cookies
async fn extract_jwt_from_cookies(cookie_jar: &CookieJar) -> Result<Claims, String> {
    // Get auth token from cookie
    let token = cookie_jar.get("auth_token")
        .ok_or("No auth token found in cookies")?
        .value();
    
    // Validate JWT
    validate_jwt(token)
        .map_err(|e| format!("Invalid JWT: {}", e))
}

/// Generate a unique client ID based on user email with UUID for multi-tab support
/// This creates a unique ID per browser tab/connection while maintaining user consistency
fn generate_client_id(email: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    let mut hasher = DefaultHasher::new();
    email.hash(&mut hasher);
    let user_hash = hasher.finish();
    
    // Add UUID for unique tab identification while keeping user hash for consistency
    let unique_id = uuid::Uuid::new_v4().to_string()[..8].to_string(); // Short UUID
    
    format!("client-{:x}-{}", user_hash, unique_id)
}

// ============================================================================
// WEBSOCKET STATISTICS ENDPOINT
// ============================================================================

/// Get WebSocket statistics (for monitoring/debugging)
pub async fn websocket_stats_handler(
    State(state): State<WebSocketState>,
) -> Result<axum::Json<serde_json::Value>, axum::http::StatusCode> {
    let stats = state.device_store.get_stats().await;
    let active_devices = state.device_store.get_active_devices().await;
    
    Ok(axum::Json(serde_json::json!({
        "websocket_stats": {
            "total_devices": stats.total_devices,
            "total_events": stats.total_events,
            "active_devices": stats.active_devices,
            "total_connections": stats.total_connections,
            "average_events_per_device": stats.average_events_per_device,
            "average_connections_per_device": stats.average_connections_per_device,
            "active_device_details": active_devices
        }
    })))
}

/// Get users currently connected to a device
pub async fn device_users_handler(
    axum::extract::Path(device_id): axum::extract::Path<String>,
    State(state): State<WebSocketState>,
    cookie_jar: CookieJar,
) -> Result<axum::Json<serde_json::Value>, axum::http::StatusCode> {
    // Authenticate user
    let _claims = match extract_jwt_from_cookies(&cookie_jar).await {
        Ok(claims) => claims,
        Err(_) => return Err(axum::http::StatusCode::UNAUTHORIZED),
    };
    
    // Get users for device with database lookup for display names
    let users = state.device_store.get_device_users_with_db(&device_id, &state.db).await;
    
    Ok(axum::Json(serde_json::json!({
        "device_id": device_id,
        "users": users
    })))
}

// ============================================================================
// WEBSOCKET CLEANUP TASK
// ============================================================================

/// Background task to clean up stale WebSocket connections
pub async fn start_cleanup_task(device_store: SharedDeviceStore) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
    
    loop {
        interval.tick().await;
        
        match device_store.cleanup_stale_connections().await {
            count if count > 0 => info!("Cleaned up {} stale WebSocket connections", count),
            _ => debug!("No stale connections to clean up"),
        }
    }
}