// Canvas event store for multiuser functionality

use crate::events::{DeviceEvent, EventWithMetadata, ServerMessage};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tracing::{info, warn, error, debug};

// User color generation system
const USER_COLORS: &[&str] = &[
    "#FF6B6B", // Red
    "#4ECDC4", // Teal
    "#45B7D1", // Blue
    "#96CEB4", // Green
    "#FFEAA7", // Yellow
    "#DDA0DD", // Plum
    "#98D8C8", // Mint
    "#F7DC6F", // Lemon
    "#BB8FCE", // Lavender
    "#85C1E9", // Sky Blue
    "#F8C471", // Orange
    "#82E0AA", // Light Green
    "#F1948A", // Salmon
    "#AED6F1", // Light Blue
    "#A9DFBF", // Pale Green
    "#F9E79F", // Pale Yellow
];

/// Generate a user color based on user_id (deterministic but well-distributed)
fn generate_user_color(user_id: &str, existing_colors: &[String]) -> String {
    // Use a better hash function based on FNV-1a algorithm for better distribution
    let hash = fnv_hash(user_id);
    let primary_index = (hash as usize) % USER_COLORS.len();
    let preferred_color = USER_COLORS[primary_index].to_string();
    
    debug!("Hash {} -> index {} -> color {} for user_id '{}'", 
           hash, primary_index, preferred_color, user_id);
    
    // If preferred color is not taken, use it
    if !existing_colors.contains(&preferred_color) {
        debug!("Preferred color {} available for user {}", preferred_color, user_id);
        return preferred_color;
    }
    
    debug!("Preferred color {} taken, finding alternative for user {}", preferred_color, user_id);
    
    // Use deterministic fallback: try colors in hash-based order, not sequential
    for i in 1..USER_COLORS.len() {
        let fallback_index = (primary_index + i) % USER_COLORS.len();
        let fallback_color = USER_COLORS[fallback_index].to_string();
        
        if !existing_colors.contains(&fallback_color) {
            debug!("Found alternative color {} (index {}) for user {}", 
                   fallback_color, fallback_index, user_id);
            return fallback_color;
        }
    }
    
    // Ultimate fallback: if all 16 colors are taken, create slight variation
    warn!("All {} colors taken! Creating variation for user {}", USER_COLORS.len(), user_id);
    create_color_variation(&preferred_color, existing_colors.len())
}

/// FNV-1a hash function for better distribution than simple polynomial hash
fn fnv_hash(input: &str) -> u32 {
    const FNV_OFFSET_BASIS: u32 = 2166136261;
    const FNV_PRIME: u32 = 16777619;
    
    let mut hash = FNV_OFFSET_BASIS;
    for byte in input.bytes() {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Create a slight color variation when all base colors are taken
fn create_color_variation(base_color: &str, variation_factor: usize) -> String {
    // Parse hex color
    if let Ok(color_num) = u32::from_str_radix(&base_color[1..], 16) {
        let r = (color_num >> 16) & 0xFF;
        let g = (color_num >> 8) & 0xFF;
        let b = color_num & 0xFF;
        
        // Apply slight modification based on variation factor
        let mod_factor = (variation_factor % 8) as u32 * 8; // 0, 8, 16, 24, 32, 40, 48, 56
        let r_mod = ((r + mod_factor) % 256).min(255);
        let g_mod = ((g + mod_factor) % 256).min(255); 
        let b_mod = ((b + mod_factor) % 256).min(255);
        
        let modified_color = format!("#{:02X}{:02X}{:02X}", r_mod, g_mod, b_mod);
        debug!("Created color variation: {} -> {} (factor: {})", base_color, modified_color, mod_factor);
        modified_color
    } else {
        // Fallback to original color if parsing fails
        base_color.to_string()
    }
}

// WebSocket client connection management

// Active WebSocket connection to a canvas
#[derive(Debug, Clone)]
pub struct ClientConnection {
    pub user_id: String,
    pub display_name: String,
    pub client_id: String,
    pub canvas_id: String,
    pub user_color: String,
    pub sender: mpsc::UnboundedSender<ServerMessage>,
}

impl ClientConnection {
    pub fn new(
        user_id: String,
        display_name: String,
        client_id: String, 
        canvas_id: String,
        user_color: String,
        sender: mpsc::UnboundedSender<ServerMessage>
    ) -> Self {
        Self {
            user_id,
            display_name,
            client_id,
            canvas_id,
            user_color,
            sender,
        }
    }
    
    // Send a message to this client
    pub fn send_message(&self, message: ServerMessage) -> Result<(), String> {
        self.sender.send(message)
            .map_err(|e| format!("Failed to send message to client {}: {}", self.client_id, e))
    }
}

// Thread-safe in-memory store for canvas events and active connections
#[derive(Debug)]
pub struct DeviceEventStore {
    // Events stored per canvas ID
    canvas_events: RwLock<HashMap<String, Vec<EventWithMetadata>>>,
    // Active client connections per canvas ID
    active_connections: RwLock<HashMap<String, Vec<ClientConnection>>>,
    // Shape selections per canvas: canvas_id -> shape_id -> (client_id, user_color)
    shape_selections: RwLock<HashMap<String, HashMap<String, (String, String)>>>,
}

impl DeviceEventStore {
    // Create a new empty event store
    pub fn new() -> Self {
        Self {
            canvas_events: RwLock::new(HashMap::new()),
            active_connections: RwLock::new(HashMap::new()),
            shape_selections: RwLock::new(HashMap::new()),
        }
    }
    
    // Event management methods
    
    // Add a new event to a canvas and broadcast to all connected clients
    pub async fn add_event(
        &self, 
        canvas_id: String, 
        event: DeviceEvent, 
        user_id: String,
        client_id: String
    ) -> Result<(), String> {
        // Validate event before storing
        event.validate().map_err(|e| format!("Invalid event: {}", e))?;
        
        // Device events don't need special tracking like shape selection
        debug!("Added device event to store: {:?}", event);
        
        // Create event with metadata
        let event_with_metadata = EventWithMetadata {
            event: event.clone(),
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            user_id: user_id.clone(),
            is_replay: None,
        };
        
        // Store event
        {
            let mut events = self.canvas_events.write().await;
            let canvas_events = events.entry(canvas_id.clone()).or_insert_with(Vec::new);
            canvas_events.push(event_with_metadata);
        }
        
        debug!("Stored event for canvas {}: {:?}", canvas_id, event);
        
        // Broadcast to all connected clients (except sender)
        self.broadcast_event(&canvas_id, event, &client_id).await?;
        
        Ok(())
    }
    
    // Get all events for a canvas (for replay when client connects)
    pub async fn get_canvas_events(&self, canvas_id: &str) -> Vec<DeviceEvent> {
        let events = self.canvas_events.read().await;
        
        match events.get(canvas_id) {
            Some(canvas_events) => {
                canvas_events.iter()
                    .map(|event_meta| event_meta.event.clone())
                    .collect()
            },
            None => {
                debug!("No events found for canvas: {}", canvas_id);
                Vec::new()
            }
        }
    }
    
    // Get device-specific information (placeholder for ESP32 device info)
    pub async fn get_device_info(&self, device_id: &str) -> Vec<DeviceEvent> {
        // For ESP32 devices, we might return device status, sensor data, etc.
        // For now, return empty - this can be extended for device-specific info
        Vec::new()
    }
    
    // Get event count for a canvas (for debugging/monitoring)
    pub async fn get_event_count(&self, canvas_id: &str) -> usize {
        let events = self.canvas_events.read().await;
        events.get(canvas_id).map(|v| v.len()).unwrap_or(0)
    }
    
    // Clear all events for a canvas (for testing or canvas reset)
    pub async fn clear_canvas_events(&self, canvas_id: &str) -> Result<(), String> {
        let mut events = self.canvas_events.write().await;
        if let Some(canvas_events) = events.get_mut(canvas_id) {
            canvas_events.clear();
            info!("Cleared all events for canvas: {}", canvas_id);
        }
        Ok(())
    }
    
    // ========================================================================
    // CONNECTION MANAGEMENT
    // ========================================================================
    
    /// Register a new client connection to a canvas
    pub async fn register_client(
        &self,
        canvas_id: String,
        user_id: String,
        display_name: String,
        client_id: String,
        sender: mpsc::UnboundedSender<ServerMessage>
    ) -> Result<Vec<DeviceEvent>, String> {
        // ATOMIC OPERATION: Generate color and add connection in single critical section
        let (user_color, is_reconnection) = {
            let mut connections = self.active_connections.write().await;
            let canvas_connections = connections.entry(canvas_id.clone()).or_insert_with(Vec::new);
            
            // Check if this user already has a color (reconnection)
            let existing_user_color = canvas_connections.iter()
                .find(|conn| conn.user_id == user_id)
                .map(|conn| conn.user_color.clone());
            
            // Only remove connection if it's the exact same client_id (true reconnection)
            // Multi-tab support: different client_ids from same user should coexist
            canvas_connections.retain(|conn| conn.client_id != client_id);
            
            // Check if this is a reconnection (user already has a color)
            let is_reconnection = existing_user_color.is_some();
            
            // Generate color only if user is truly new
            let user_color = if let Some(color) = existing_user_color {
                debug!("User {} reconnecting with existing color: {}", user_id, color);
                color
            } else {
                // Collect already assigned colors in this canvas (per unique user_id)
                let mut user_colors: std::collections::HashMap<String, String> = std::collections::HashMap::new();
                for conn in canvas_connections.iter() {
                    // Only count each user_id once, regardless of how many connections they have
                    user_colors.insert(conn.user_id.clone(), conn.user_color.clone());
                }
                let existing_colors: Vec<String> = user_colors.values().cloned().collect();
                
                debug!("Canvas {}: Existing colors for {} users: {:?}", 
                       canvas_id, user_colors.len(), existing_colors);
                
                // Generate color for this new user
                generate_user_color(&user_id, &existing_colors)
            };
            
            info!("Color {} for user {} on canvas {}", 
                  user_color, user_id, canvas_id);
            
            // Create and add new connection atomically
            let connection = ClientConnection::new(
                user_id.clone(), 
                display_name.clone(), 
                client_id.clone(), 
                canvas_id.clone(), 
                user_color.clone(), 
                sender
            );
            canvas_connections.push(connection);
            
            (user_color, is_reconnection)
        };
        
        info!("Client {} registered for canvas {} (user: {})", client_id, canvas_id, user_id);
        
        // Broadcast user joined event only for truly new users (not reconnections)
        if !is_reconnection {
            let user_joined_event = crate::events::DeviceEvent::user_joined(user_id.clone(), display_name.clone(), user_color.clone());
            if let Err(e) = self.broadcast_event(&canvas_id, user_joined_event, &client_id).await {
                error!("Failed to broadcast user joined event: {}", e);
            }
        } else {
            debug!("Skipping userJoined broadcast for reconnecting user: {}", user_id);
            // Multi-Tab Fix: Send refresh signal to update connection counts in other clients
            let refresh_event = crate::events::DeviceEvent::user_joined("USER_COUNT_REFRESH".to_string(), "".to_string(), "".to_string());
            if let Err(e) = self.broadcast_event(&canvas_id, refresh_event, &client_id).await {
                error!("Failed to broadcast connection count refresh event: {}", e);
            }
        }
        
        // Return all existing events for replay
        let device_events = self.get_canvas_events(&canvas_id).await;
        let events = device_events;
        
        debug!("Sending {} events to newly registered client {}", 
               events.len(), client_id);
        
        Ok(events)
    }
    
    /// Unregister a client from a canvas
    pub async fn unregister_client(&self, canvas_id: &str, client_id: &str) -> Result<(), String> {
        let mut connection_to_remove: Option<ClientConnection> = None;
        
        // First, find and remove the connection while keeping track of user info
        {
            let mut connections = self.active_connections.write().await;
            
            if let Some(canvas_connections) = connections.get_mut(canvas_id) {
                let initial_count = canvas_connections.len();
                
                // Find the connection we're about to remove
                if let Some(conn) = canvas_connections.iter().find(|conn| conn.client_id == client_id) {
                    connection_to_remove = Some(conn.clone());
                }
                
                // Remove the connection
                canvas_connections.retain(|conn| conn.client_id != client_id);
                
                if canvas_connections.len() < initial_count {
                    info!("Client {} unregistered from canvas {}", client_id, canvas_id);
                } else {
                    warn!("Attempted to unregister non-existent client {} from canvas {}", client_id, canvas_id);
                }
                
                // Clean up empty canvas entries
                if canvas_connections.is_empty() {
                    connections.remove(canvas_id);
                    debug!("Removed empty canvas connection list for: {}", canvas_id);
                }
            }
        }
        
        // Broadcast user left event if we found the connection
        if let Some(removed_connection) = connection_to_remove {
            // Check if this user still has other connections to this canvas
            let user_still_connected = {
                let connections = self.active_connections.read().await;
                if let Some(canvas_connections) = connections.get(canvas_id) {
                    canvas_connections.iter().any(|conn| conn.user_id == removed_connection.user_id)
                } else {
                    false
                }
            };
            
            // Only broadcast user left event if they have no more connections to this canvas
            if !user_still_connected {
                let user_left_event = crate::events::DeviceEvent::user_left(
                    removed_connection.user_id,
                    removed_connection.display_name,
                    removed_connection.user_color
                );
                if let Err(e) = self.broadcast_event(canvas_id, user_left_event, client_id).await {
                    error!("Failed to broadcast user left event: {}", e);
                }
            } else {
                // Multi-Tab Fix: Send refresh signal to update connection counts when user reduces tabs
                let refresh_event = crate::events::DeviceEvent::user_left("USER_COUNT_REFRESH".to_string(), "".to_string(), "".to_string());
                if let Err(e) = self.broadcast_event(canvas_id, refresh_event, client_id).await {
                    error!("Failed to broadcast connection count refresh event: {}", e);
                }
            }
        }
        
        // ESP32 devices don't have shape selections to clean up
        debug!("Client {} disconnected from device {}", client_id, canvas_id);
        
        Ok(())
    }
    
    /// Get count of active connections for a canvas
    pub async fn get_connection_count(&self, canvas_id: &str) -> usize {
        let connections = self.active_connections.read().await;
        connections.get(canvas_id).map(|v| v.len()).unwrap_or(0)
    }
    
    /// Get all active canvases with their connection counts
    pub async fn get_active_canvases(&self) -> HashMap<String, usize> {
        let connections = self.active_connections.read().await;
        connections.iter()
            .map(|(canvas_id, connections)| (canvas_id.clone(), connections.len()))
            .collect()
    }
    
    /// Get all users currently connected to a canvas
    pub async fn get_canvas_users(&self, canvas_id: &str) -> Vec<CanvasUser> {
        let connections = self.active_connections.read().await;
        
        if let Some(canvas_connections) = connections.get(canvas_id) {
            // Group connections by user_id to count multiple connections and get color
            let mut user_map: std::collections::HashMap<String, (String, String, usize)> = std::collections::HashMap::new();
            
            for connection in canvas_connections {
                let entry = user_map.entry(connection.user_id.clone())
                    .or_insert((connection.display_name.clone(), connection.user_color.clone(), 0));
                entry.2 += 1;
            }
            
            user_map.into_iter()
                .map(|(user_id, (display_name, user_color, connection_count))| CanvasUser {
                    user_id,
                    display_name,
                    connection_count,
                    user_color,
                })
                .collect()
        } else {
            Vec::new()
        }
    }
    
    /// Get all users currently connected to a canvas with database lookup for display names
    pub async fn get_canvas_users_with_db(&self, canvas_id: &str, db: &crate::database::DatabaseManager) -> Vec<CanvasUser> {
        let connections = self.active_connections.read().await;
        
        if let Some(canvas_connections) = connections.get(canvas_id) {
            // Group connections by user_id to count multiple connections and get color
            let mut user_map: std::collections::HashMap<String, (String, usize)> = std::collections::HashMap::new();
            
            for connection in canvas_connections {
                let entry = user_map.entry(connection.user_id.clone())
                    .or_insert((connection.user_color.clone(), 0));
                entry.1 += 1;
            }
            
            // Collect results with database lookup for display names
            let mut users = Vec::new();
            for (user_id, (user_color, connection_count)) in user_map {
                // Try to get display name from database
                let display_name = match db.get_user_by_id(&user_id).await {
                    Ok(Some(user)) => user.display_name,
                    _ => user_id.clone() // Fallback to user_id
                };
                
                users.push(CanvasUser {
                    user_id,
                    display_name,
                    connection_count,
                    user_color,
                });
            }
            
            users
        } else {
            Vec::new()
        }
    }
    
    // ========================================================================
    // EVENT BROADCASTING
    // ========================================================================
    
    /// Broadcast an event to all connected clients on a canvas (except sender)
    /// Multi-tab support: Sends to all clients including other tabs of same user
    async fn broadcast_event(
        &self, 
        canvas_id: &str, 
        event: DeviceEvent, 
        sender_client_id: &str
    ) -> Result<(), String> {
        let connections = self.active_connections.read().await;
        
        if let Some(canvas_connections) = connections.get(canvas_id) {
            let message = ServerMessage::device_events(
                canvas_id.to_string(),
                vec![event]
            );
            
            let mut successful_sends = 0;
            let mut failed_sends = 0;
            
            for connection in canvas_connections {
                // Don't send event back to the exact sender client
                // But do send to other tabs of the same user (different client_id)
                if connection.client_id == sender_client_id {
                    continue;
                }
                
                match connection.send_message(message.clone()) {
                    Ok(()) => successful_sends += 1,
                    Err(e) => {
                        error!("Failed to broadcast to client {}: {}", connection.client_id, e);
                        failed_sends += 1;
                    }
                }
            }
            
            debug!("Broadcasted event to {} clients on canvas {} ({} failed)", 
                   successful_sends, canvas_id, failed_sends);
            
            // TODO: Clean up failed connections in a background task
        }
        
        Ok(())
    }
    
    /// Get the base user identifier from a client_id (for multi-tab support)
    fn get_user_hash_from_client_id(client_id: &str) -> Option<String> {
        // Extract user hash from client_id format: "client-{hash}-{uuid}"
        if let Some(parts) = client_id.strip_prefix("client-") {
            if let Some(hash_end) = parts.rfind('-') {
                return Some(format!("client-{}", &parts[..hash_end]));
            }
        }
        None
    }
    
    // ========================================================================
    // CLEANUP & MAINTENANCE
    // ========================================================================
    
    /// Remove stale connections (connections where the sender channel is closed)
    pub async fn cleanup_stale_connections(&self) -> usize {
        let mut connections = self.active_connections.write().await;
        let mut removed_count = 0;
        
        // Check each canvas
        let canvas_ids: Vec<String> = connections.keys().cloned().collect();
        
        for canvas_id in canvas_ids {
            if let Some(canvas_connections) = connections.get_mut(&canvas_id) {
                let initial_count = canvas_connections.len();
                
                // Keep only connections with open channels
                canvas_connections.retain(|conn| !conn.sender.is_closed());
                
                let removed_for_canvas = initial_count - canvas_connections.len();
                removed_count += removed_for_canvas;
                
                if removed_for_canvas > 0 {
                    debug!("Removed {} stale connections from canvas {}", removed_for_canvas, canvas_id);
                }
                
                // Remove empty canvas entries
                if canvas_connections.is_empty() {
                    connections.remove(&canvas_id);
                }
            }
        }
        
        if removed_count > 0 {
            info!("Cleaned up {} stale connections", removed_count);
        }
        
        removed_count
    }
    
    /// Get storage statistics for monitoring
    pub async fn get_stats(&self) -> CanvasStoreStats {
        let events = self.canvas_events.read().await;
        let connections = self.active_connections.read().await;
        
        let total_events: usize = events.values().map(|v| v.len()).sum();
        let total_connections: usize = connections.values().map(|v| v.len()).sum();
        
        CanvasStoreStats {
            total_canvases: events.len(),
            total_events,
            active_canvases: connections.len(),
            total_connections,
            average_events_per_canvas: if events.is_empty() { 0.0 } else { total_events as f64 / events.len() as f64 },
            average_connections_per_canvas: if connections.is_empty() { 0.0 } else { total_connections as f64 / connections.len() as f64 },
        }
    }
}

// ============================================================================
// STATISTICS & MONITORING
// ============================================================================

#[derive(Debug, Clone)]
pub struct CanvasStoreStats {
    pub total_canvases: usize,
    pub total_events: usize,
    pub active_canvases: usize,
    pub total_connections: usize,
    pub average_events_per_canvas: f64,
    pub average_connections_per_canvas: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CanvasUser {
    pub user_id: String,
    pub display_name: String,
    pub connection_count: usize,
    pub user_color: String,
}

impl Default for DeviceEventStore {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// CONVENIENCE TYPE ALIASES
// ============================================================================

pub type SharedCanvasStore = Arc<DeviceEventStore>;

/// Create a new shared canvas store instance
pub fn create_shared_store() -> SharedCanvasStore {
    Arc::new(DeviceEventStore::new())
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{Shape, ShapeType, Point};
    use tokio::sync::mpsc;
    
    async fn create_test_store() -> DeviceEventStore {
        DeviceEventStore::new()
    }
    
    fn create_test_event() -> DeviceEvent {
        let shape = Shape::new_line(
            "test-line-1".to_string(),
            Point::new(0.0, 0.0),
            Point::new(100.0, 100.0),
            "000000".to_string(),
            1
        );
        DeviceEvent::add_shape(shape)
    }
    
    #[tokio::test]
    async fn test_add_and_get_events() {
        let store = create_test_store().await;
        let canvas_id = "test-canvas".to_string();
        let user_id = "test-user".to_string();
        let event = create_test_event();
        
        // Add event
        store.add_event(canvas_id.clone(), event.clone(), user_id.clone(), "test-client-1".to_string()).await.unwrap();
        
        // Get events
        let events = store.get_canvas_events(&canvas_id).await;
        assert_eq!(events.len(), 1);
        
        // Check event count
        let count = store.get_event_count(&canvas_id).await;
        assert_eq!(count, 1);
    }
    
    #[tokio::test]
    async fn test_client_registration() {
        let store = create_test_store().await;
        let canvas_id = "test-canvas".to_string();
        let user_id = "test-user".to_string();
        let client_id = "test-client".to_string();
        
        let (sender, _receiver) = mpsc::unbounded_channel();
        
        // Register client
        let events = store.register_client(
            canvas_id.clone(),
            user_id.clone(),
            user_id.clone(), // display_name = user_id for test
            client_id.clone(),
            sender
        ).await.unwrap();
        
        assert_eq!(events.len(), 0); // No events initially
        
        // Check connection count
        let count = store.get_connection_count(&canvas_id).await;
        assert_eq!(count, 1);
        
        // Unregister client
        store.unregister_client(&canvas_id, &client_id).await.unwrap();
        
        let count = store.get_connection_count(&canvas_id).await;
        assert_eq!(count, 0);
    }
    
    #[tokio::test]
    async fn test_event_replay_on_registration() {
        let store = create_test_store().await;
        let canvas_id = "test-canvas".to_string();
        let user_id = "test-user".to_string();
        let client_id = "test-client".to_string();
        
        // Add some events first
        let event1 = create_test_event();
        let event2 = create_test_event();
        
        store.add_event(canvas_id.clone(), event1, user_id.clone(), "test-client-1".to_string()).await.unwrap();
        store.add_event(canvas_id.clone(), event2, user_id.clone(), "test-client-2".to_string()).await.unwrap();
        
        // Register client and check if events are returned
        let (sender, _receiver) = mpsc::unbounded_channel();
        let events = store.register_client(
            canvas_id,
            user_id.clone(),
            user_id.clone(), // display_name = user_id for test
            client_id,
            sender
        ).await.unwrap();
        
        assert_eq!(events.len(), 2);
    }
}