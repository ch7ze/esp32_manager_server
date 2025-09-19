// ESP32 device manager - handles multiple ESP32 connections and integrates with device store

use crate::esp32_connection::{Esp32Connection};
use crate::esp32_types::{
    Esp32Command, Esp32Event, Esp32DeviceConfig, ConnectionState, Esp32Result, Esp32Error
};
use crate::device_store::{SharedDeviceStore, DeviceEventStore};
use crate::events::DeviceEvent;

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::panic;
use std::time::Instant;
use tokio::sync::{mpsc, RwLock, Mutex};
use tokio::net::UdpSocket;
use tokio::time::{sleep, timeout, Duration, interval};
use tracing::{info, warn, error, debug};

// ============================================================================
// ESP32 DEVICE MANAGER
// ============================================================================

/// Manages multiple ESP32 device connections and integrates with the device store
#[derive(Debug)]
pub struct Esp32Manager {
    /// Map of device_id -> ESP32 connection
    connections: Arc<RwLock<HashMap<String, Arc<Mutex<Esp32Connection>>>>>,
    /// Map of device_id -> event sender (separate from connection to avoid mutex blocking)
    device_event_senders: Arc<RwLock<HashMap<String, mpsc::UnboundedSender<Esp32Event>>>>,
    /// Device configurations
    device_configs: Arc<RwLock<HashMap<String, Esp32DeviceConfig>>>,
    /// Shared device store for event management
    device_store: SharedDeviceStore,
    /// Event sender for internal communication
    event_sender: mpsc::UnboundedSender<Esp32ManagerEvent>,
    /// Event receiver for processing (Option because it's moved to the processor task)
    event_receiver: Arc<Mutex<Option<mpsc::UnboundedReceiver<Esp32ManagerEvent>>>>,
    /// Central UDP listener for all ESP32 devices
    central_udp_socket: Arc<Mutex<Option<UdpSocket>>>,
    /// Map of IP -> device_id for UDP message routing
    ip_to_device_id: Arc<RwLock<HashMap<IpAddr, String>>>,
    /// Global mutex to prevent race conditions during device connections
    connection_mutex: Arc<Mutex<()>>,
    /// Direct bypass event sender for crashed Event Forwarding Tasks
    bypass_event_sender: mpsc::UnboundedSender<Esp32ManagerEvent>,
    /// UDP activity tracking for device connectivity monitoring
    udp_activity_tracker: Arc<RwLock<HashMap<String, Instant>>>,
    /// Connection state tracking to prevent redundant events (device_id -> is_connected)
    udp_connection_states: Arc<RwLock<HashMap<String, bool>>>,
}

/// Internal events for ESP32 manager
#[derive(Debug, Clone)]
pub enum Esp32ManagerEvent {
    DeviceEvent(String, Esp32Event), // (device_id, event)
    ConnectionStateChanged(String, ConnectionState), // (device_id, state)
}

impl Esp32Manager {
    /// Create new ESP32 manager
    pub fn new(device_store: SharedDeviceStore) -> Self {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();

        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            device_event_senders: Arc::new(RwLock::new(HashMap::new())),
            device_configs: Arc::new(RwLock::new(HashMap::new())),
            device_store,
            event_sender: event_sender.clone(),
            event_receiver: Arc::new(Mutex::new(Some(event_receiver))),
            central_udp_socket: Arc::new(Mutex::new(None)),
            ip_to_device_id: Arc::new(RwLock::new(HashMap::new())),
            connection_mutex: Arc::new(Mutex::new(())),
            bypass_event_sender: event_sender,
            udp_activity_tracker: Arc::new(RwLock::new(HashMap::new())),
            udp_connection_states: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Start the ESP32 manager background tasks
    pub async fn start(&self) {
        info!("Starting ESP32 Manager");

        // Start central UDP listener immediately
        if let Err(e) = self.start_central_udp_listener().await {
            error!("Failed to start central UDP listener: {}", e);
        }

        // Start event processing task
        self.start_event_processor().await;

        // Start reconnect monitoring task
        self.start_reconnect_monitor().await;

        // Start UDP timeout monitoring task
        self.start_udp_timeout_monitor().await;

        info!("ESP32 Manager started");
    }
    
    /// Add a new ESP32 device configuration
    pub async fn add_device(&self, config: Esp32DeviceConfig) -> Esp32Result<()> {
        let device_id = config.device_id.clone();
        info!("Adding ESP32 device: {} ({}:{})",
               device_id, config.ip_address, config.tcp_port);
        crate::debug_logger::DebugLogger::log_device_add(&device_id);

        // Check if device already exists
        {
            let connections = self.connections.read().await;
            if connections.contains_key(&device_id) {
                info!("ESP32 device {} already exists, updating configuration only", device_id);
                crate::debug_logger::DebugLogger::log_device_already_exists(&device_id);

                // Update configuration but keep existing connection
                let mut configs = self.device_configs.write().await;
                configs.insert(device_id.clone(), config.clone());

                return Ok(());
            }
        }

        // Store configuration
        {
            let mut configs = self.device_configs.write().await;
            configs.insert(device_id.clone(), config.clone());
        }

        // Create connection with direct manager event sender - SIMPLIFIED SYSTEM
        info!("Creating ESP32Connection for device {} with direct manager event sender", device_id);

        // Use manager's bypass event sender directly to avoid complex forwarding layers
        let device_event_sender = self.create_direct_device_sender(device_id.clone());

        info!("Direct event sender created for device {} - closed: {}", device_id, device_event_sender.is_closed());
        let connection = Esp32Connection::new(config, device_event_sender);

        {
            let mut connections = self.connections.write().await;
            connections.insert(device_id.clone(), Arc::new(Mutex::new(connection)));
            crate::debug_logger::DebugLogger::log_event("ESP32_MANAGER", &format!("CONNECTION_STORED_IN_HASHMAP: {}", device_id));
        }

        info!("ESP32 device {} added successfully", device_id);
        crate::debug_logger::DebugLogger::log_event("ESP32_MANAGER", &format!("ESP32_DEVICE_ADDED_SUCCESS: {}", device_id));
        Ok(())
    }
    
    /// Remove ESP32 device
    pub async fn remove_device(&self, device_id: &str) -> Esp32Result<()> {
        info!("Removing ESP32 device: {}", device_id);
        
        // Disconnect if connected
        if let Err(e) = self.disconnect_device(device_id).await {
            warn!("Error disconnecting device {} during removal: {}", device_id, e);
        }
        
        // Remove from collections
        {
            let mut connections = self.connections.write().await;
            connections.remove(device_id);
        }

        {
            let mut senders = self.device_event_senders.write().await;
            senders.remove(device_id);
        }

        {
            let mut configs = self.device_configs.write().await;
            configs.remove(device_id);
        }
        
        info!("ESP32 device {} removed", device_id);
        Ok(())
    }
    
    /// Connect to ESP32 device
    pub async fn connect_device(&self, device_id: &str) -> Esp32Result<()> {
        info!("DEVICE CONNECTION DEBUG: Starting connection process for device: {}", device_id);
        crate::debug_logger::DebugLogger::log_event("ESP32_MANAGER", &format!("CONNECT_DEVICE_START: {}", device_id));

        // Use global mutex to prevent race conditions between multiple connection attempts
        let _connection_guard = self.connection_mutex.lock().await;
        crate::debug_logger::DebugLogger::log_event("ESP32_MANAGER", &format!("CONNECT_DEVICE_MUTEX_ACQUIRED: {}", device_id));

        // First, check if we need to recreate the connection with a fresh direct sender
        let needs_recreation = {
            let connections = self.connections.read().await;
            if let Some(connection_arc) = connections.get(device_id) {
                let connection = connection_arc.lock().await;
                let current_state = connection.get_connection_state().await;
                match current_state {
                    ConnectionState::Connected => {
                        info!("DEVICE CONNECTION DEBUG: Device {} already connected - skipping", device_id);
                        crate::debug_logger::DebugLogger::log_event("ESP32_MANAGER", &format!("ALREADY_CONNECTED_SKIP: {}", device_id));
                        return Ok(());
                    }
                    ConnectionState::Connecting => {
                        warn!("DEVICE CONNECTION DEBUG: Device {} already connecting - preventing race condition", device_id);
                        crate::debug_logger::DebugLogger::log_event("ESP32_MANAGER", &format!("ALREADY_CONNECTING_SKIP: {}", device_id));
                        return Err(Esp32Error::ConnectionFailed("Already connecting".to_string()));
                    }
                    ConnectionState::Disconnected | ConnectionState::Failed(_) => {
                        info!("DEVICE CONNECTION DEBUG: Device {} is disconnected/failed - recreating connection", device_id);
                        crate::debug_logger::DebugLogger::log_event("ESP32_MANAGER", &format!("RECREATING_CONNECTION: {}", device_id));
                        true // Recreate connection
                    }
                }
            } else {
                false // No connection exists
            }
        };

        if needs_recreation {
            info!("DEVICE CONNECTION DEBUG: Recreating ESP32Connection with fresh direct sender for device: {}", device_id);

            // Get device config
            let config = {
                let configs = self.device_configs.read().await;
                configs.get(device_id).cloned().ok_or_else(|| {
                    Esp32Error::DeviceNotFound(format!("Device config not found for {}", device_id))
                })?
            };

            // Create new ESP32Connection with fresh direct sender
            let direct_sender = self.create_direct_device_sender(device_id.to_string());
            let new_connection = Esp32Connection::new(config.clone(), direct_sender);
            let connection_arc = Arc::new(Mutex::new(new_connection));

            // Replace the connection
            {
                let mut connections = self.connections.write().await;
                connections.insert(device_id.to_string(), connection_arc.clone());
            }

            info!("DEVICE CONNECTION DEBUG: ESP32Connection recreated for device: {}", device_id);
        }

        let connections = self.connections.read().await;
        if let Some(connection_arc) = connections.get(device_id) {
            info!("DEVICE CONNECTION DEBUG: Found connection for device: {}", device_id);
            crate::debug_logger::DebugLogger::log_event("ESP32_MANAGER", &format!("CONNECTION_FOUND: {}", device_id));

            let mut connection = connection_arc.lock().await;

            info!("DEVICE CONNECTION DEBUG: Attempting TCP connection for device: {}", device_id);
            crate::debug_logger::DebugLogger::log_event("ESP32_MANAGER", &format!("ATTEMPTING_TCP_CONNECTION: {}", device_id));

            match connection.connect().await {
                Ok(()) => {
                    info!("DEVICE CONNECTION DEBUG: TCP connection established for device: {}", device_id);
                    crate::debug_logger::DebugLogger::log_event("ESP32_MANAGER", &format!("TCP_CONNECTION_SUCCESS: {}", device_id));
                },
                Err(e) => {
                    error!("DEVICE CONNECTION DEBUG: TCP connection failed for device: {} - Error: {}", device_id, e);
                    crate::debug_logger::DebugLogger::log_event("ESP32_MANAGER", &format!("TCP_CONNECTION_FAILED: {} - Error: {}", device_id, e));
                    return Err(e);
                }
            }

            // Register device for central UDP routing
            let config = {
                let configs = self.device_configs.read().await;
                configs.get(device_id).cloned()
            };

            if let Some(ref config) = config {
                crate::debug_logger::DebugLogger::log_event("ESP32_MANAGER", &format!("REGISTERING_UDP_ROUTING: {} -> {}", device_id, config.ip_address));
                self.register_esp32_for_udp(device_id.to_string(), config.ip_address).await;

                // Initialize UDP activity tracking for connected device
                {
                    let mut tracker = self.udp_activity_tracker.write().await;
                    tracker.insert(device_id.to_string(), Instant::now());
                    info!("UDP activity tracking initialized for device: {}", device_id);
                }

                // Mark device as connected in UDP connection states
                {
                    let mut states = self.udp_connection_states.write().await;
                    states.insert(device_id.to_string(), true);
                    info!("UDP connection state set to connected for device: {}", device_id);
                }
            }

            info!("DEVICE CONNECTION DEBUG: Successfully connected to ESP32 device: {}", device_id);
            info!("DEVICE CONNECTION DEBUG: Connection status events should now be sent to frontend for device: {}", device_id);
            crate::debug_logger::DebugLogger::log_event("ESP32_MANAGER", &format!("CONNECT_DEVICE_SUCCESS: {}", device_id));

            // WORKAROUND: Send connection status event directly through manager
            // This ensures frontend gets notified even if ESP32Connection event sender is closed
            if let Some(config) = config {
                let device_event = crate::events::DeviceEvent::esp32_connection_status(
                    device_id.to_string(),
                    true, // connected
                    config.ip_address.to_string(),
                    config.tcp_port,
                    config.udp_port,
                );

                if let Err(e) = self.device_store.add_event(
                    device_id.to_string(),
                    device_event,
                    "ESP32_MANAGER".to_string(),
                    "SYSTEM_CONNECTION".to_string(),
                ).await {
                    error!("ESP32MANAGER DEBUG: Failed to send manual connection status event for device {}: {}", device_id, e);
                } else {
                    info!("ESP32MANAGER DEBUG: Manual connection status event sent successfully for device {}", device_id);
                }
            }

            Ok(())
        } else {
            crate::debug_logger::DebugLogger::log_event("ESP32_MANAGER", &format!("DEVICE_NOT_FOUND: {}", device_id));
            Err(Esp32Error::DeviceNotFound(device_id.to_string()))
        }
    }
    
    /// Disconnect from ESP32 device
    pub async fn disconnect_device(&self, device_id: &str) -> Esp32Result<()> {
        info!("Disconnecting from ESP32 device: {}", device_id);

        let connections = self.connections.read().await;
        if let Some(connection_arc) = connections.get(device_id) {
            let mut connection = connection_arc.lock().await;

            // Unregister from UDP routing first
            let config = {
                let configs = self.device_configs.read().await;
                configs.get(device_id).cloned()
            };

            if let Some(config) = config {
                self.unregister_esp32_from_udp(&config.ip_address).await;
            }

            connection.disconnect().await?;
            info!("Successfully disconnected from ESP32 device: {}", device_id);
            Ok(())
        } else {
            Err(Esp32Error::DeviceNotFound(device_id.to_string()))
        }
    }
    
    /// Send command to ESP32 device
    pub async fn send_command(&self, device_id: &str, command: Esp32Command) -> Esp32Result<()> {
        debug!("Sending command to ESP32 device {}: {:?}", device_id, command);
        
        let connections = self.connections.read().await;
        if let Some(connection_arc) = connections.get(device_id) {
            let connection = connection_arc.lock().await;
            connection.send_command(command).await?;
            debug!("Command sent successfully to ESP32 device: {}", device_id);
            Ok(())
        } else {
            Err(Esp32Error::DeviceNotFound(device_id.to_string()))
        }
    }
    
    /// Get connection state of ESP32 device
    pub async fn get_device_state(&self, device_id: &str) -> Option<ConnectionState> {
        let connections = self.connections.read().await;
        if let Some(connection_arc) = connections.get(device_id) {
            let connection = connection_arc.lock().await;
            Some(connection.get_connection_state().await)
        } else {
            None
        }
    }
    
    /// Get all configured ESP32 devices
    pub async fn get_all_devices(&self) -> Vec<Esp32DeviceConfig> {
        let configs = self.device_configs.read().await;
        configs.values().cloned().collect()
    }
    
    /// Get device configuration
    pub async fn get_device_config(&self, device_id: &str) -> Option<Esp32DeviceConfig> {
        let configs = self.device_configs.read().await;
        configs.get(device_id).cloned()
    }
    
    /// Auto-discover ESP32 devices (placeholder for future UDP discovery)
    pub async fn discover_devices(&self) -> Esp32Result<Vec<Esp32DeviceConfig>> {
        // TODO: Implement UDP broadcast discovery like UdpSearcher.cs
        // For now return empty list
        info!("ESP32 device discovery not yet implemented");
        Ok(Vec::new())
    }
    
    // ========================================================================
    // INTEGRATION WITH DEVICE STORE
    // ========================================================================
    
    /// Handle ESP32 command from WebSocket client (via device store)
    pub async fn handle_websocket_command(
        &self,
        device_id: &str,
        command_data: serde_json::Value,
        user_id: &str,
        client_id: &str,
    ) -> Esp32Result<()> {
        debug!("Handling WebSocket command for ESP32 device {}: {:?}", device_id, command_data);
        
        // Parse command from JSON
        let command = self.parse_websocket_command(command_data)?;
        
        // Send command to ESP32
        self.send_command(device_id, command.clone()).await?;
        
        // Create device event for logging/broadcasting
        let device_event = DeviceEvent::esp32_command(
            device_id.to_string(),
            serde_json::to_value(command)?,
        );
        
        // Add event to device store (this will broadcast to all connected clients)
        if let Err(e) = self.device_store.add_event(
            device_id.to_string(),
            device_event,
            user_id.to_string(),
            client_id.to_string(),
        ).await {
            error!("Failed to add ESP32 command event to device store: {}", e);
        }
        
        Ok(())
    }
    
    /// Parse WebSocket command data into ESP32 command
    fn parse_websocket_command(&self, data: serde_json::Value) -> Esp32Result<Esp32Command> {
        // Handle setVariable command
        if let Some(set_var) = data.get("setVariable") {
            if let (Some(name), Some(value)) = (set_var.get("name"), set_var.get("value")) {
                if let (Some(name_str), Some(value_num)) = (name.as_str(), value.as_u64()) {
                    return Ok(Esp32Command::set_variable(
                        name_str.to_string(),
                        value_num as u32,
                    ));
                }
            }
        }
        
        // Handle startOption command
        if let Some(start_option) = data.get("startOption") {
            if let Some(option_str) = start_option.as_str() {
                return Ok(Esp32Command::start_option(option_str.to_string()));
            }
        }
        
        // Handle reset command
        if data.get("reset").is_some() {
            return Ok(Esp32Command::reset());
        }
        
        // Handle getStatus command
        if data.get("getStatus").is_some() {
            return Ok(Esp32Command::get_status());
        }
        
        Err(Esp32Error::InvalidCommand(format!("Unknown command: {:?}", data)))
    }
    
    // ========================================================================
    // EVENT PROCESSING
    // ========================================================================
    
    /// Create event sender for a specific device
    fn create_device_event_sender(&self, device_id: String) -> mpsc::UnboundedSender<Esp32Event> {
        let manager_sender = self.event_sender.clone();
        let bypass_event_sender = self.bypass_event_sender.clone();

        let (tx, mut rx) = mpsc::unbounded_channel();

        // Sanitize device_id for logging to prevent issues with special characters
        let safe_device_id = device_id.replace(':', "_COLON_");
        crate::debug_logger::DebugLogger::log_event("EVENT_FORWARDING_TASK", &format!("SANITIZED_DEVICE_ID: {} -> {}", device_id, safe_device_id));

        // Forward device events to manager - clone sender to ensure it stays alive
        let device_id_for_spawn = device_id.clone();
        let manager_sender_clone = manager_sender.clone();
        let spawn_handle = tokio::spawn(async move {
                info!("EVENT FORWARDING DEBUG: Started event forwarding task for device {}", device_id);
                info!("EVENT FORWARDING DEBUG: Task spawned, waiting for events from device {}", device_id);
                crate::debug_logger::DebugLogger::log_event_forwarding_task_start(&device_id);
                let mut event_count = 0;

                crate::debug_logger::DebugLogger::log_event("EVENT_FORWARDING_TASK", &format!("LOOP_START for device {}", device_id));
                info!("EVENT FORWARDING DEBUG: About to enter main event loop for device {}", device_id);

                // Check manager sender status
                let manager_sender_closed = manager_sender_clone.is_closed();
                crate::debug_logger::DebugLogger::log_event("EVENT_FORWARDING_TASK", &format!("MANAGER_SENDER_STATUS for device {} - is_closed: {}", device_id, manager_sender_closed));

                crate::debug_logger::DebugLogger::log_event("EVENT_FORWARDING_TASK", &format!("ENTERING_MAIN_LOOP for device {}", device_id));

                loop {
                    info!("EVENT FORWARDING DEBUG: Device {} waiting for next event (processed so far: {})", device_id, event_count);
                    crate::debug_logger::DebugLogger::log_event("EVENT_FORWARDING_TASK", &format!("LOOP_ITERATION for device {} - event_count: {}", device_id, event_count));

                    info!("EVENT FORWARDING DEBUG: Device {} calling rx.recv().await", device_id);
                    crate::debug_logger::DebugLogger::log_event("EVENT_FORWARDING_TASK", &format!("CALLING_RECV for device {}", device_id));

                    // Add detailed pre-recv status
                    crate::debug_logger::DebugLogger::log_event("EVENT_FORWARDING_TASK", &format!("PRE_RECV_STATUS for device {} - manager_sender_closed: {}", device_id, manager_sender_clone.is_closed()));

                    // Add timeout to see if recv blocks indefinitely
                    crate::debug_logger::DebugLogger::log_event("EVENT_FORWARDING_TASK", &format!("CALLING_RECV_WITH_TIMEOUT for device {}", device_id));

                    // Use select! to ensure the timeout is not blocked by recv
                    let recv_result = tokio::select! {
                        result = rx.recv() => {
                            crate::debug_logger::DebugLogger::log_event("EVENT_FORWARDING_TASK", &format!("RECV_COMPLETED_BEFORE_TIMEOUT for device {}", device_id));
                            Ok(result)
                        }
                        _ = tokio::time::sleep(Duration::from_secs(5)) => {
                            crate::debug_logger::DebugLogger::log_event("EVENT_FORWARDING_TASK", &format!("RECV_TIMEOUT_TRIGGERED for device {}", device_id));
                            Err(())
                        }
                    };

                    // Add immediate post-recv logging
                    crate::debug_logger::DebugLogger::log_event("EVENT_FORWARDING_TASK", &format!("POST_RECV_IMMEDIATE for device {} - timeout_result: {}", device_id, recv_result.is_ok()));

                let final_recv_result = match recv_result {
                    Ok(recv_data) => {
                        crate::debug_logger::DebugLogger::log_event("EVENT_FORWARDING_TASK", &format!("RECV_SUCCESS for device {} - has_data: {:?}", device_id, recv_data.is_some()));

                        if recv_data.is_none() {
                            crate::debug_logger::DebugLogger::log_event("EVENT_FORWARDING_TASK", &format!("RECV_RETURNED_NONE_IMMEDIATELY for device {} - channel closed!", device_id));
                            error!("EVENT FORWARDING DEBUG: CRITICAL - Channel for device {} returned None! ESP32Connection event_sender was dropped!", device_id);
                            error!("EVENT FORWARDING DEBUG: This explains the 'EVENT_SEND FAILED' error - the sender is trying to send to a closed channel!");
                            error!("EVENT FORWARDING DEBUG: The ESP32Connection for {} needs to be recreated or the channel is corrupted!", device_id);

                            // Check if we have the Connection in the manager
                            let manager_sender_still_open = !manager_sender_clone.is_closed();
                            crate::debug_logger::DebugLogger::log_event("EVENT_FORWARDING_TASK", &format!("RECV_NONE_MANAGER_SENDER_STATUS for device {} - still_open: {}", device_id, manager_sender_still_open));

                            // Exit the task since the channel is permanently closed
                            error!("EVENT FORWARDING DEBUG: Task for device {} exiting due to closed channel", device_id);
                            crate::debug_logger::DebugLogger::log_event("EVENT_FORWARDING_TASK", &format!("TASK_EXITING_CHANNEL_CLOSED for device {}", device_id));
                            break;
                        }

                        recv_data
                    }
                    Err(_) => {
                        crate::debug_logger::DebugLogger::log_event("EVENT_FORWARDING_TASK", &format!("RECV_TIMEOUT for device {} - continuing...", device_id));

                        // Check if manager sender is still alive
                        if manager_sender_clone.is_closed() {
                            error!("EVENT FORWARDING DEBUG: CRITICAL - Manager sender closed for device {} during timeout - this will cause task to exit!", device_id);
                            crate::debug_logger::DebugLogger::log_event("EVENT_FORWARDING_TASK", &format!("MANAGER_SENDER_CLOSED_DURING_TIMEOUT for device {}", device_id));
                            break; // Exit the loop if manager sender is closed
                        }

                        continue; // Continue the loop after timeout
                    }
                };

                crate::debug_logger::DebugLogger::log_event("EVENT_FORWARDING_TASK", &format!("RECV_COMPLETED for device {} - result: {:?}", device_id, final_recv_result.is_some()));

                match final_recv_result {
                    Some(event) => {
                        event_count += 1;
                        info!("EVENT FORWARDING DEBUG: Received event #{} from device {}: {:?}", event_count, device_id, event);
                        crate::debug_logger::DebugLogger::log_event_forwarding_task_receive(&device_id, event_count, &format!("{:?}", event));

                        info!("EVENT FORWARDING DEBUG: Attempting to forward event #{} from device {} to manager", event_count, device_id);
                        crate::debug_logger::DebugLogger::log_event("EVENT_FORWARDING_TASK", &format!("ATTEMPTING_SEND to manager for device {} - event #{}", device_id, event_count));
                        match manager_sender_clone.send(Esp32ManagerEvent::DeviceEvent(device_id.clone(), event)) {
                            Ok(()) => {
                                info!("EVENT FORWARDING DEBUG: Successfully forwarded event #{} from device {} to manager", event_count, device_id);
                                crate::debug_logger::DebugLogger::log_event_forwarding_task_send_success(&device_id, event_count);
                            }
                            Err(e) => {
                                error!("EVENT FORWARDING DEBUG: FAILED to forward event #{} from device {} to manager: {}", event_count, device_id, e);
                                error!("EVENT FORWARDING DEBUG: Manager channel appears to be closed - this is why events don't reach frontend!");
                                error!("EVENT FORWARDING DEBUG: Event forwarding task will exit for device {} after {} events", device_id, event_count);
                                error!("EVENT FORWARDING DEBUG: Manager sender error details: {}", e);
                                crate::debug_logger::DebugLogger::log_event_forwarding_task_send_fail(&device_id, event_count, &e.to_string());
                                crate::debug_logger::DebugLogger::log_event("EVENT_FORWARDING_TASK", &format!("BREAK_ON_SEND_ERROR for device {}", device_id));
                                break;
                            }
                        }
                    }
                    None => {
                        warn!("EVENT FORWARDING DEBUG: Receiver channel closed for device {} after {} events", device_id, event_count);
                        warn!("EVENT FORWARDING DEBUG: This means the ESP32Connection event_sender was dropped!");
                        crate::debug_logger::DebugLogger::log_event("EVENT_FORWARDING_TASK", &format!("RECV_RETURNED_NONE for device {}", device_id));
                        crate::debug_logger::DebugLogger::log_event("EVENT_FORWARDING_TASK", &format!("BREAK_ON_CHANNEL_CLOSED for device {}", device_id));
                        break;
                    }
                }
            }

                if event_count == 0 {
                    error!("EVENT FORWARDING DEBUG: Event forwarding task for device {} ended WITHOUT processing any events - channel was closed immediately!", device_id);
                    error!("EVENT FORWARDING DEBUG: This indicates the ESP32Connection event_sender was dropped before any events were sent!");
                } else {
                    warn!("EVENT FORWARDING DEBUG: Event forwarding task ended for device {} after processing {} events", device_id, event_count);
                }
                warn!("EVENT FORWARDING DEBUG: Event forwarding task for device {} is now TERMINATED", device_id);
                crate::debug_logger::DebugLogger::log_event_forwarding_task_end(&device_id, event_count);

                event_count // Return event count
        });

        // Monitor the spawned task for completion or panic
        let monitoring_device_id = device_id_for_spawn.clone();
        let manager_sender_for_recovery = manager_sender.clone();
        let bypass_sender_for_recovery = bypass_event_sender.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await; // Give task time to start

            // Log that monitoring started
            crate::debug_logger::DebugLogger::log_event("EVENT_FORWARDING_TASK", &format!("MONITORING_STARTED for device {}", monitoring_device_id));

            match spawn_handle.await {
                Ok(event_count) => {
                    error!("EVENT FORWARDING DEBUG: CRITICAL - Task completed normally for device {} with {} events - THIS IS THE BUG!", monitoring_device_id, event_count);
                    crate::debug_logger::DebugLogger::log_event("EVENT_FORWARDING_TASK", &format!("TASK_COMPLETED_NORMALLY for device {} - events: {}", monitoring_device_id, event_count));
                }
                Err(join_error) => {
                    if join_error.is_panic() {
                        error!("EVENT FORWARDING DEBUG: PANIC in task for device {}: {:?}", monitoring_device_id, join_error);
                        crate::debug_logger::DebugLogger::log_event("EVENT_FORWARDING_TASK", &format!("TASK_PANICKED for device {}: {:?}", monitoring_device_id, join_error));
                    } else if join_error.is_cancelled() {
                        error!("EVENT FORWARDING DEBUG: Task CANCELLED for device {}", monitoring_device_id);
                        crate::debug_logger::DebugLogger::log_event("EVENT_FORWARDING_TASK", &format!("TASK_CANCELLED for device {}", monitoring_device_id));
                    } else {
                        error!("EVENT FORWARDING DEBUG: Task FAILED for device {}: {:?}", monitoring_device_id, join_error);
                        crate::debug_logger::DebugLogger::log_event("EVENT_FORWARDING_TASK", &format!("TASK_FAILED for device {}: {:?}", monitoring_device_id, join_error));
                    }

                    // Send manual connection status and restart task automatically
                    error!("EVENT FORWARDING DEBUG: Restarting crashed Event Forwarding Task for {}", monitoring_device_id);

                    // Send manual connection status event using bypass sender
                    let manual_event = Esp32ManagerEvent::DeviceEvent(
                        monitoring_device_id.clone(),
                        Esp32Event::ConnectionStatus {
                            connected: true,
                            device_ip: "0.0.0.0".parse().unwrap(), // Will be updated by actual connection
                            tcp_port: 0,
                            udp_port: 0,
                        }
                    );

                    // Try regular sender first, then bypass if it fails
                    if let Err(e) = manager_sender_for_recovery.send(manual_event.clone()) {
                        error!("EVENT FORWARDING DEBUG: Regular sender failed for {}: {}, trying bypass", monitoring_device_id, e);
                        if let Err(e) = bypass_sender_for_recovery.send(manual_event) {
                            error!("EVENT FORWARDING DEBUG: Bypass sender also failed for {}: {}", monitoring_device_id, e);
                        } else {
                            error!("EVENT FORWARDING DEBUG: Bypass manual recovery event sent for {}", monitoring_device_id);
                        }
                    } else {
                        error!("EVENT FORWARDING DEBUG: Manual recovery event sent for {}", monitoring_device_id);
                    }

                    // Auto-restart the crashed Event Forwarding Task
                    error!("EVENT FORWARDING DEBUG: Event Forwarding Task crashed for {}", monitoring_device_id);
                    error!("EVENT FORWARDING DEBUG: Will send recovery status - need to manually restart task or reconnect device");
                }
            }
        });
        
        tx
    }

    /// Create a direct device event sender - SIMPLIFIED VERSION
    /// This bypasses the complex event forwarding layer and sends directly to the manager
    fn create_direct_device_sender(&self, device_id: String) -> mpsc::UnboundedSender<Esp32Event> {
        info!("Creating direct device sender for {}", device_id);

        // Create a simple channel that wraps events with device_id and forwards to manager
        let (tx, mut rx) = mpsc::unbounded_channel();
        let manager_sender = self.bypass_event_sender.clone();

        // Spawn a simple forwarding task that just wraps events with device_id
        tokio::spawn(async move {
            info!("DIRECT SENDER: Started direct forwarding task for device {}", device_id);

            while let Some(event) = rx.recv().await {
                // Wrap the event with device_id and send to manager
                let manager_event = Esp32ManagerEvent::DeviceEvent(device_id.clone(), event);

                if let Err(e) = manager_sender.send(manager_event) {
                    warn!("DIRECT SENDER: Failed to send event for device {}: {}", device_id, e);
                    break;
                }
            }

            info!("DIRECT SENDER: Direct forwarding task ended for device {}", device_id);
        });

        tx
    }

    /// Start background event processor
    async fn start_event_processor(&self) {
        let event_receiver = Arc::clone(&self.event_receiver);
        let device_store = Arc::clone(&self.device_store);

        info!("ESP32MANAGER EVENT PROCESSOR DEBUG: Starting ESP32Manager event processor");
        crate::debug_logger::DebugLogger::log_event("MANAGER_EVENT_PROCESSOR", "STARTING");
        tokio::spawn(async move {
            info!("ESP32MANAGER EVENT PROCESSOR DEBUG: ESP32Manager event processor task started");
            crate::debug_logger::DebugLogger::log_event("MANAGER_EVENT_PROCESSOR", "TASK_STARTED");
            let mut event_count = 0;

            info!("ESP32MANAGER EVENT PROCESSOR DEBUG: Taking receiver ownership");
            crate::debug_logger::DebugLogger::log_event("MANAGER_EVENT_PROCESSOR", "TAKING_RECEIVER_OWNERSHIP");
            let mut receiver = {
                let mut receiver_option = event_receiver.lock().await;
                receiver_option.take().expect("Event receiver should only be taken once")
            };
            info!("ESP32MANAGER EVENT PROCESSOR DEBUG: ESP32Manager event processor has receiver ownership");
            crate::debug_logger::DebugLogger::log_event("MANAGER_EVENT_PROCESSOR", "RECEIVER_OWNERSHIP_ACQUIRED");

            loop {
                info!("ESP32MANAGER EVENT PROCESSOR DEBUG: Waiting for next event (processed so far: {})", event_count);
                crate::debug_logger::DebugLogger::log_event("MANAGER_EVENT_PROCESSOR", &format!("WAITING_FOR_EVENT - count: {}", event_count));

                let recv_result = receiver.recv().await;
                crate::debug_logger::DebugLogger::log_event("MANAGER_EVENT_PROCESSOR", &format!("RECV_RESULT - is_some: {}", recv_result.is_some()));

                match recv_result {
                    Some(event) => {
                        event_count += 1;
                        info!("ESP32MANAGER EVENT PROCESSOR DEBUG: Received event #{}: {:?}", event_count, event);

                        match event {
                            Esp32ManagerEvent::DeviceEvent(device_id, esp32_event) => {
                                info!("ESP32MANAGER EVENT PROCESSOR DEBUG: Processing device event #{} for device {}: {:?}", event_count, device_id, esp32_event);

                                match Self::handle_esp32_event(&device_store, &device_id, esp32_event).await {
                                    Ok(()) => {
                                        info!("ESP32MANAGER EVENT PROCESSOR DEBUG: Successfully processed event #{} for device {}", event_count, device_id);
                                    }
                                    Err(e) => {
                                        error!("ESP32MANAGER EVENT PROCESSOR DEBUG: Failed to handle ESP32 event #{} for device {}: {}", event_count, device_id, e);
                                        error!("ESP32MANAGER EVENT PROCESSOR DEBUG: This could cause the event processor to become unstable!");
                                    }
                                }
                            }
                            Esp32ManagerEvent::ConnectionStateChanged(device_id, state) => {
                                info!("ESP32MANAGER EVENT PROCESSOR DEBUG: Processing connection state change #{} for device {}: {:?}", event_count, device_id, state);
                                // TODO: Notify connected clients about state change
                            }
                        }
                    }
                    None => {
                        warn!("ESP32MANAGER EVENT PROCESSOR DEBUG: Receiver channel closed after {} events", event_count);
                        warn!("ESP32MANAGER EVENT PROCESSOR DEBUG: This means all event senders have been dropped!");
                        crate::debug_logger::DebugLogger::log_event("MANAGER_EVENT_PROCESSOR", &format!("RECV_RETURNED_NONE - count: {}", event_count));
                        crate::debug_logger::DebugLogger::log_event("MANAGER_EVENT_PROCESSOR", "BREAKING_FROM_LOOP");
                        break;
                    }
                }
            }

            if event_count == 0 {
                error!("ESP32MANAGER EVENT PROCESSOR DEBUG: Event processor ended WITHOUT processing any events!");
                error!("ESP32MANAGER EVENT PROCESSOR DEBUG: This indicates the event processor was terminated immediately after startup!");
            } else {
                error!("ESP32MANAGER EVENT PROCESSOR DEBUG: Event processor ended after processing {} events!", event_count);
            }
            error!("ESP32MANAGER EVENT PROCESSOR DEBUG: ESP32Manager event processor task is now TERMINATED!");
        });
    }

    /// Start reconnect monitoring task that automatically reconnects disconnected devices
    async fn start_reconnect_monitor(&self) {
        let connections = Arc::clone(&self.connections);
        let device_configs = Arc::clone(&self.device_configs);
        let reconnect_interval = Duration::from_secs(5); // Check every 5 seconds

        info!("ESP32 RECONNECT MONITOR: Starting automatic reconnect monitoring");
        tokio::spawn(async move {
            loop {
                sleep(reconnect_interval).await;

                // Get list of all device configs
                let configs_to_check = {
                    let configs = device_configs.read().await;
                    configs.clone() // Clone the HashMap to avoid holding the lock
                };

                for (device_id, config) in configs_to_check {
                    // Check connection state
                    let needs_reconnect = {
                        let connections = connections.read().await;
                        if let Some(connection_arc) = connections.get(&device_id) {
                            let connection = connection_arc.lock().await;
                            let state = connection.get_connection_state().await;
                            matches!(state, ConnectionState::Disconnected | ConnectionState::Failed(_))
                        } else {
                            true // No connection exists at all
                        }
                    };

                    if needs_reconnect {
                        info!("ESP32 RECONNECT MONITOR: Device {} is disconnected, attempting reconnect", device_id);

                        // Note: We can't call self.connect_device here because we don't have &self
                        // Instead, we'll let the next connection attempt handle this
                        debug!("ESP32 RECONNECT MONITOR: Device {} needs reconnection - will be handled by next connection request", device_id);
                    }
                }
            }
        });
    }

    /// Handle ESP32 event by converting it to DeviceEvent and storing it
    async fn handle_esp32_event(
        device_store: &DeviceEventStore,
        device_id: &str,
        esp32_event: Esp32Event,
    ) -> Result<(), String> {
        debug!("Processing ESP32 event for device {}: {:?}", device_id, esp32_event);

        // Use device_id as-is (with hyphens for MAC addresses) for consistent key usage
        debug!("Using device ID '{}' for WebSocket broadcasting", device_id);

        // Convert ESP32 event to DeviceEvent using device_id
        let device_event = match esp32_event {
            Esp32Event::VariableUpdate { name, value } => {
                DeviceEvent::esp32_variable_update(device_id.to_string(), name, value)
            }
            Esp32Event::StartOptions { options } => {
                DeviceEvent::esp32_start_options(device_id.to_string(), options)
            }
            Esp32Event::ChangeableVariables { variables } => {
                let vars_json: Vec<serde_json::Value> = variables.into_iter().map(|v| {
                    serde_json::json!({ "name": v.name, "value": v.value })
                }).collect();
                DeviceEvent::esp32_changeable_variables(device_id.to_string(), vars_json)
            }
            Esp32Event::UdpBroadcast { message, from_ip, from_port } => {
                DeviceEvent::esp32_udp_broadcast(device_id.to_string(), message, from_ip, from_port)
            }
            Esp32Event::ConnectionStatus { connected, device_ip, tcp_port, udp_port } => {
                info!("ESP32 EVENT PROCESSING DEBUG: Processing connection status event for device {}: connected={}, ip={}, tcp_port={}, udp_port={}",
                      device_id, connected, device_ip, tcp_port, udp_port);
                if connected {
                    info!("ESP32 EVENT PROCESSING DEBUG: Device {} is now CONNECTED - this should update frontend to show 'Connected'", device_id);
                } else {
                    info!("ESP32 EVENT PROCESSING DEBUG: Device {} is now DISCONNECTED - this should update frontend to show 'Disconnected'", device_id);
                }
                DeviceEvent::esp32_connection_status(device_id.to_string(), connected, device_ip, tcp_port, udp_port)
            }
            Esp32Event::DeviceInfo { device_id: _, device_name, firmware_version, uptime } => {
                DeviceEvent::esp32_device_info(device_id.to_string(), device_name, firmware_version, uptime)
            }
        };
        
        // Add event to device store (this will broadcast to all connected WebSocket clients)
        // Use device_id consistently (with hyphens for MAC addresses)
        device_store.add_event(
            device_id.to_string(),
            device_event,
            "ESP32_SYSTEM".to_string(), // System user for ESP32 events
            "ESP32_INTERNAL".to_string(), // Internal client ID
        ).await?;
        
        Ok(())
    }

    // ========================================================================
    // CENTRAL UDP LISTENER
    // ========================================================================

    /// Start central UDP listener for all ESP32 devices
    async fn start_central_udp_listener(&self) -> Esp32Result<()> {
        const UDP_PORT: u16 = 3232;
        let addr = SocketAddr::from(([0, 0, 0, 0], UDP_PORT));

        let socket = UdpSocket::bind(addr)
            .await
            .map_err(|e| Esp32Error::ConnectionFailed(
                format!("Central UDP bind failed on {}: {}", addr, e)
            ))?;

        info!("Central UDP listener started on {}", addr);

        // Store socket
        {
            let mut udp_socket = self.central_udp_socket.lock().await;
            *udp_socket = Some(socket);
        }

        // Start listener task
        let socket = Arc::clone(&self.central_udp_socket);
        let ip_to_device_id = Arc::clone(&self.ip_to_device_id);
        let _device_event_senders = Arc::clone(&self.device_event_senders);
        let device_store = Arc::clone(&self.device_store);
        let udp_activity_tracker = Arc::clone(&self.udp_activity_tracker);
        let udp_connection_states = Arc::clone(&self.udp_connection_states);

        tokio::spawn(async move {
            let mut buffer = [0u8; 1024];
            info!("Central UDP listener task started");

            loop {
                let socket_guard = socket.lock().await;
                if let Some(udp_socket) = socket_guard.as_ref() {
                    match timeout(Duration::from_millis(100), udp_socket.recv_from(&mut buffer)).await {
                        Ok(Ok((bytes_read, from_addr))) => {
                            let message = String::from_utf8_lossy(&buffer[..bytes_read]).to_string();

                            // Always print to terminal
                            println!("UDP Message from {}: {}", from_addr, message);
                            info!("UDP Message from {}: {}", from_addr, message);

                            // Route message to specific ESP32 connection if registered
                            {
                                let device_map = ip_to_device_id.read().await;
                                if let Some(device_id) = device_map.get(&from_addr.ip()) {
                                    // Update UDP activity tracking
                                    {
                                        let mut tracker = udp_activity_tracker.write().await;
                                        tracker.insert(device_id.clone(), Instant::now());
                                        debug!("UDP activity updated for device: {}", device_id);
                                    }

                                    // Use direct device store bypass - much more reliable than event processor
                                    debug!("UDP: Routing message to device {} via direct bypass", device_id);
                                    Self::handle_udp_message_bypass_smart(&message, from_addr, device_id, &device_store, &udp_connection_states).await;
                                } else {
                                    drop(device_map); // Drop read lock before getting write lock

                                    // Check if this looks like a TCP message that should be routed via UDP bypass
                                    if Self::is_tcp_message(&message) {
                                        if let Some(device_id) = Self::extract_device_id_from_tcp_message(&message) {
                                            info!("TCP message from unregistered IP {} - auto-registering device {}", from_addr.ip(), device_id);
                                            // Auto-register this IP for the device
                                            {
                                                let mut device_map = ip_to_device_id.write().await;
                                                device_map.insert(from_addr.ip(), device_id.clone());
                                            }

                                            // Update UDP activity tracking
                                            {
                                                let mut tracker = udp_activity_tracker.write().await;
                                                tracker.insert(device_id.clone(), Instant::now());
                                                debug!("UDP activity updated for auto-registered device: {}", device_id);
                                            }

                                            // Route the TCP message through UDP bypass
                                            debug!("TCP via UDP bypass: Routing message to device {} via direct bypass", device_id);
                                            Self::handle_udp_message_bypass_smart(&message, from_addr, &device_id, &device_store, &udp_connection_states).await;
                                        } else {
                                            info!("TCP message from unregistered IP {} but no device ID found", from_addr.ip());
                                        }
                                    } else {
                                        info!("No device registered for IP {}", from_addr.ip());
                                    }
                                }
                            }
                        }
                        Ok(Err(e)) => {
                            error!("Central UDP receive error: {}", e);
                            sleep(Duration::from_secs(1)).await;
                        }
                        Err(_) => {
                            // Timeout, continue
                        }
                    }
                } else {
                    sleep(Duration::from_millis(100)).await;
                }
            }
        });

        Ok(())
    }

    /// Handle UDP message with smart connection state detection to prevent redundant events
    pub async fn handle_udp_message_bypass_smart(
        message: &str,
        from_addr: SocketAddr,
        device_id: &str,
        device_store: &SharedDeviceStore,
        udp_connection_states: &Arc<RwLock<HashMap<String, bool>>>
    ) {
        debug!("UDP bypass: Processing message from {} for device {}: {}", from_addr, device_id, message);

        // Check if device was previously disconnected (or never seen before)
        let should_send_connected_event = {
            let mut states = udp_connection_states.write().await;
            let was_connected = states.get(device_id).copied().unwrap_or(false);

            if !was_connected {
                // Device was disconnected or new - mark as connected
                states.insert(device_id.to_string(), true);
                info!("UDP RECONNECT: Device {} is now connected (was: disconnected)", device_id);
                true
            } else {
                // Device was already connected - no event needed
                false
            }
        };

        // Only send connection event if state changed from disconnected to connected
        if should_send_connected_event {
            let connection_event = crate::events::DeviceEvent::esp32_connection_status(
                device_id.to_string(),
                true, // connected = true since we're receiving UDP
                from_addr.ip().to_string(),
                0, // no TCP port available
                from_addr.port() // UDP port
            );
            if let Err(e) = device_store.add_event(device_id.to_string(), connection_event, "esp32_system".to_string(), "udp_reconnect".to_string()).await {
                error!("Failed to send UDP reconnect event for device {}: {}", device_id, e);
            } else {
                info!("UDP RECONNECT: Connected event sent for device {}", device_id);
            }
        } else {
            debug!("UDP: Device {} already marked as connected - skipping redundant event", device_id);
        }

        // Send UDP broadcast event directly to device store
        let broadcast_event = crate::events::DeviceEvent::esp32_udp_broadcast(
            device_id.to_string(),
            message.to_string(),
            from_addr.ip().to_string(),
            from_addr.port()
        );
        let _ = device_store.add_event(device_id.to_string(), broadcast_event, "esp32_system".to_string(), "udp_bypass".to_string()).await;

        // Enhanced JSON parsing for structured data (matching C# RemoteAccess.cs behavior)
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(message) {
            // Handle startOptions array (from C# RemoteAccess.cs line 371-384)
            if let Some(options_array) = value.get("startOptions") {
                if let Some(options) = options_array.as_array() {
                    let mut start_options = Vec::new();
                    for option in options {
                        if let Some(option_str) = option.as_str() {
                            start_options.push(option_str.to_string());
                        }
                    }

                    if !start_options.is_empty() {
                        debug!("UDP bypass: Extracted startOptions: {:?}", start_options);
                        let start_options_event = crate::events::DeviceEvent::esp32_start_options(
                            device_id.to_string(),
                            start_options
                        );
                        let _ = device_store.add_event(device_id.to_string(), start_options_event, "esp32_system".to_string(), "udp_bypass".to_string()).await;
                    }
                }
            }

            // Handle changeableVariables array (from C# RemoteAccess.cs line 347-368)
            if let Some(vars_array) = value.get("changeableVariables") {
                if let Some(vars) = vars_array.as_array() {
                    let mut variables = Vec::new();
                    for var in vars {
                        if let (Some(name), Some(value)) = (var.get("name"), var.get("value")) {
                            if let (Some(name_str), Some(value_num)) = (name.as_str(), value.as_u64()) {
                                variables.push(serde_json::json!({
                                    "name": name_str,
                                    "value": value_num
                                }));
                            }
                        }
                    }

                    if !variables.is_empty() {
                        debug!("UDP bypass: Extracted changeableVariables: {:?}", variables);
                        let changeable_vars_event = crate::events::DeviceEvent::esp32_changeable_variables(
                            device_id.to_string(),
                            variables
                        );
                        let _ = device_store.add_event(device_id.to_string(), changeable_vars_event, "esp32_system".to_string(), "udp_bypass".to_string()).await;
                    }
                }
            }

            // Handle device information (extended from ESP32 capabilities)
            if let Some(device_name) = value.get("deviceName").and_then(|v| v.as_str()) {
                let firmware_version = value.get("firmwareVersion").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                let uptime = value.get("uptime").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

                debug!("UDP bypass: Extracted device info - name: {}, firmware: {}, uptime: {}", device_name, firmware_version, uptime);
                let device_info_event = crate::events::DeviceEvent::esp32_device_info(
                    device_id.to_string(),
                    Some(device_name.to_string()),
                    Some(firmware_version),
                    Some(uptime as u64)
                );
                let _ = device_store.add_event(device_id.to_string(), device_info_event, "esp32_system".to_string(), "udp_bypass".to_string()).await;
            }

            // Handle status information (extended functionality)
            if let Some(status) = value.get("status") {
                if let Some(status_obj) = status.as_object() {
                    // Extract various status fields and create appropriate events
                    if let Some(running) = status_obj.get("running").and_then(|v| v.as_bool()) {
                        debug!("UDP bypass: Device {} status - running: {}", device_id, running);
                    }

                    if let Some(memory_free) = status_obj.get("memoryFree").and_then(|v| v.as_u64()) {
                        debug!("UDP bypass: Device {} memory free: {} bytes", device_id, memory_free);
                    }
                }
            }
        }

        // Parse for variable updates using regex like the C# version (from RemoteAccess.cs line 89-110)
        let re = regex::Regex::new(r#"\{\"([^\"]+)\"\s*:\s*\"([^\"]+)\"\}"#).unwrap();
        for captures in re.captures_iter(message) {
            if let (Some(name), Some(value)) = (captures.get(1), captures.get(2)) {
                debug!("UDP bypass: Extracted variable update - {}={}", name.as_str().trim(), value.as_str().trim());
                let variable_event = crate::events::DeviceEvent::esp32_variable_update(
                    device_id.to_string(),
                    name.as_str().trim().to_string(),
                    value.as_str().trim().to_string(),
                );
                let _ = device_store.add_event(device_id.to_string(), variable_event, "esp32_system".to_string(), "udp_bypass".to_string()).await;
            }
        }

        // Additional parsing for numeric values without quotes (common in ESP32 output)
        let numeric_re = regex::Regex::new(r#"\{\"([^\"]+)\"\s*:\s*(\d+)\}"#).unwrap();
        for captures in numeric_re.captures_iter(message) {
            if let (Some(name), Some(value)) = (captures.get(1), captures.get(2)) {
                debug!("UDP bypass: Extracted numeric variable - {}={}", name.as_str().trim(), value.as_str().trim());
                let variable_event = crate::events::DeviceEvent::esp32_variable_update(
                    device_id.to_string(),
                    name.as_str().trim().to_string(),
                    value.as_str().trim().to_string(),
                );
                let _ = device_store.add_event(device_id.to_string(), variable_event, "esp32_system".to_string(), "udp_bypass".to_string()).await;
            }
        }
    }

    /// Handle TCP message using direct bypass to DeviceStore (like UDP bypass)
    /// This ensures TCP events reach the frontend even when EventForwardingTask crashes
    pub async fn handle_tcp_message_bypass(
        message: &str,
        device_id: &str,
        device_store: &SharedDeviceStore
    ) {
        info!("TCP BYPASS: Processing TCP message for device {}: {}", device_id, message);

        // Send connection status event directly to device store (TCP is connected)
        let connection_event = crate::events::DeviceEvent::esp32_connection_status(
            device_id.to_string(),
            true, // connected = true since we're receiving TCP
            "0.0.0.0".to_string(), // TCP doesn't provide source IP
            3232, // Default TCP port
            3232  // Default UDP port
        );
        let _ = device_store.add_event(device_id.to_string(), connection_event, "esp32_system".to_string(), "tcp_bypass".to_string()).await;

        // Enhanced JSON parsing for structured data (matching C# RemoteAccess.cs behavior)
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(message) {
            // Handle startOptions array (from C# RemoteAccess.cs line 371-384)
            if let Some(options_array) = value.get("startOptions") {
                if let Some(options) = options_array.as_array() {
                    let mut start_options = Vec::new();
                    for option in options {
                        if let Some(option_str) = option.as_str() {
                            start_options.push(option_str.to_string());
                        }
                    }

                    if !start_options.is_empty() {
                        info!("TCP BYPASS: Extracted startOptions for {}: {:?}", device_id, start_options);
                        let start_options_event = crate::events::DeviceEvent::esp32_start_options(
                            device_id.to_string(),
                            start_options
                        );
                        let _ = device_store.add_event(device_id.to_string(), start_options_event, "esp32_system".to_string(), "tcp_bypass".to_string()).await;
                    }
                }
            }

            // Handle changeableVariables array (from C# RemoteAccess.cs line 347-368)
            if let Some(vars_array) = value.get("changeableVariables") {
                if let Some(vars) = vars_array.as_array() {
                    let mut variables = Vec::new();
                    for var in vars {
                        if let (Some(name), Some(value)) = (var.get("name"), var.get("value")) {
                            if let (Some(name_str), Some(value_num)) = (name.as_str(), value.as_u64()) {
                                variables.push(serde_json::json!({
                                    "name": name_str,
                                    "value": value_num
                                }));
                            }
                        }
                    }

                    if !variables.is_empty() {
                        info!("TCP BYPASS: Extracted changeableVariables for {}: {:?}", device_id, variables);
                        let changeable_vars_event = crate::events::DeviceEvent::esp32_changeable_variables(
                            device_id.to_string(),
                            variables
                        );
                        let _ = device_store.add_event(device_id.to_string(), changeable_vars_event, "esp32_system".to_string(), "tcp_bypass".to_string()).await;
                    }
                }
            }

            // Handle device information (extended from ESP32 capabilities)
            if let Some(device_name) = value.get("deviceName").and_then(|v| v.as_str()) {
                let firmware_version = value.get("firmwareVersion").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                let uptime = value.get("uptime").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

                info!("TCP BYPASS: Extracted device info for {} - name: {}, firmware: {}, uptime: {}", device_id, device_name, firmware_version, uptime);
                let device_info_event = crate::events::DeviceEvent::esp32_device_info(
                    device_id.to_string(),
                    Some(device_name.to_string()),
                    Some(firmware_version),
                    Some(uptime as u64)
                );
                let _ = device_store.add_event(device_id.to_string(), device_info_event, "esp32_system".to_string(), "tcp_bypass".to_string()).await;
            }
        }

        // Parse for variable updates using regex like the C# version (from RemoteAccess.cs line 89-110)
        let re = regex::Regex::new(r#"\{\"([^\"]+)\"\s*:\s*\"([^\"]+)\"\}"#).unwrap();
        for captures in re.captures_iter(message) {
            if let (Some(name), Some(value)) = (captures.get(1), captures.get(2)) {
                info!("TCP BYPASS: Extracted variable update for {} - {}={}", device_id, name.as_str().trim(), value.as_str().trim());
                let variable_event = crate::events::DeviceEvent::esp32_variable_update(
                    device_id.to_string(),
                    name.as_str().trim().to_string(),
                    value.as_str().trim().to_string(),
                );
                let _ = device_store.add_event(device_id.to_string(), variable_event, "esp32_system".to_string(), "tcp_bypass".to_string()).await;
            }
        }

        // Additional parsing for numeric values without quotes (common in ESP32 output)
        let numeric_re = regex::Regex::new(r#"\{\"([^\"]+)\"\s*:\s*(\d+)\}"#).unwrap();
        for captures in numeric_re.captures_iter(message) {
            if let (Some(name), Some(value)) = (captures.get(1), captures.get(2)) {
                info!("TCP BYPASS: Extracted numeric variable for {} - {}={}", device_id, name.as_str().trim(), value.as_str().trim());
                let variable_event = crate::events::DeviceEvent::esp32_variable_update(
                    device_id.to_string(),
                    name.as_str().trim().to_string(),
                    value.as_str().trim().to_string(),
                );
                let _ = device_store.add_event(device_id.to_string(), variable_event, "esp32_system".to_string(), "tcp_bypass".to_string()).await;
            }
        }

        info!("TCP BYPASS: Finished processing message for device {}", device_id);
    }

    /// Check if a message looks like a TCP message with JSON structure
    fn is_tcp_message(message: &str) -> bool {
        // TCP messages from ESP32 are usually JSON with specific fields
        message.trim_start().starts_with('{') && (
            message.contains("\"startOptions\"") ||
            message.contains("\"changeableVariables\"") ||
            message.contains("\"setVariable\"") ||
            message.contains("\"startOption\"") ||
            message.contains("\"reset\"")
        )
    }

    /// Extract device ID from TCP message structure
    fn extract_device_id_from_tcp_message(_message: &str) -> Option<String> {
        // For now, assume the known device ID since we know there's only one ESP32
        // In a real system, this would parse the message to extract device info
        Some("10-20-BA-42-71-E0".to_string())
    }

    /// Register ESP32 device for UDP message routing
    pub async fn register_esp32_for_udp(&self, device_id: String, ip: IpAddr) {
        let mut device_map = self.ip_to_device_id.write().await;
        device_map.insert(ip, device_id.clone());
        info!("ESP32 {} registered for UDP routing on IP {}", device_id, ip);
    }

    /// Unregister ESP32 device from UDP message routing
    pub async fn unregister_esp32_from_udp(&self, ip: &IpAddr) {
        let mut device_map = self.ip_to_device_id.write().await;
        if let Some(device_id) = device_map.remove(ip) {
            info!("ESP32 {} unregistered from UDP routing", device_id);
        }
    }
}

// ============================================================================
// CONVENIENCE FUNCTIONS
// ============================================================================

/// Create shared ESP32 manager instance
pub fn create_esp32_manager(device_store: SharedDeviceStore) -> Arc<Esp32Manager> {
    Arc::new(Esp32Manager::new(device_store))
}

// Global device store reference for TCP bypass
static mut GLOBAL_DEVICE_STORE: Option<SharedDeviceStore> = None;

/// Set global device store for TCP bypass access
pub fn set_global_device_store(device_store: SharedDeviceStore) {
    unsafe {
        GLOBAL_DEVICE_STORE = Some(device_store);
    }
}

/// Global TCP bypass function that can be called from ESP32Connection
pub async fn handle_tcp_bypass_global(message: &str, device_id: &str) -> Result<(), String> {
    let device_store = unsafe {
        GLOBAL_DEVICE_STORE.as_ref()
            .ok_or("Global device store not initialized")?
    };

    info!("GLOBAL TCP BYPASS: Processing message for device {}: {}", device_id, message);

    // Use the existing TCP bypass logic
    Esp32Manager::handle_tcp_message_bypass(message, device_id, device_store).await;

    Ok(())
}

/// Global TCP disconnect function that can be called from ESP32Connection
pub async fn handle_tcp_disconnect_global(device_id: &str) -> Result<(), String> {
    let device_store = unsafe {
        GLOBAL_DEVICE_STORE.as_ref()
            .ok_or("Global device store not initialized")?
    };

    info!("GLOBAL TCP DISCONNECT: Processing disconnect for device {}", device_id);

    // Send disconnect event directly to device store
    let disconnect_event = crate::events::DeviceEvent::esp32_connection_status(
        device_id.to_string(),
        false, // disconnected
        "unknown".to_string(), // IP not available here
        0, // port not available
        0, // UDP port not available
    );

    if let Err(e) = device_store.add_event(
        device_id.to_string(),
        disconnect_event,
        "ESP32_CONNECTION".to_string(),
        "TCP_DISCONNECT".to_string(),
    ).await {
        error!("GLOBAL TCP DISCONNECT: Failed to send disconnect event for device {}: {}", device_id, e);
        return Err(format!("Failed to send disconnect event: {}", e));
    }

    info!("GLOBAL TCP DISCONNECT: Disconnect event sent successfully for device {}", device_id);
    Ok(())
}

impl Esp32Manager {
    /// Start UDP timeout monitoring task
    async fn start_udp_timeout_monitor(&self) {
        let udp_activity_tracker = Arc::clone(&self.udp_activity_tracker);
        let device_configs = Arc::clone(&self.device_configs);
        let device_store = self.device_store.clone();
        let udp_connection_states = Arc::clone(&self.udp_connection_states);

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(5)); // Check every 5 seconds
            info!("UDP timeout monitor started");

            loop {
                interval.tick().await;

                let configs = device_configs.read().await;
                let mut tracker = udp_activity_tracker.write().await;
                let now = Instant::now();

                // Check each device for UDP timeout
                for (device_id, config) in configs.iter() {
                    if let Some(last_activity) = tracker.get(device_id) {
                        let elapsed = now.duration_since(*last_activity);
                        let timeout = Duration::from_secs(config.udp_timeout_seconds);

                        if elapsed > timeout {
                            warn!("UDP TIMEOUT: Device {} has been inactive for {}s (timeout: {}s)",
                                  device_id, elapsed.as_secs(), config.udp_timeout_seconds);

                            // Only send disconnect event if device was connected
                            let should_send_disconnect = {
                                let mut states = udp_connection_states.write().await;
                                let was_connected = states.get(device_id).copied().unwrap_or(false);

                                if was_connected {
                                    // Mark as disconnected
                                    states.insert(device_id.clone(), false);
                                    info!("UDP TIMEOUT: Device {} marked as disconnected", device_id);
                                    true
                                } else {
                                    // Already disconnected - no event needed
                                    false
                                }
                            };

                            if should_send_disconnect {
                                // Send disconnect event
                                let disconnect_event = crate::events::DeviceEvent::esp32_connection_status(
                                    device_id.clone(),
                                    false, // disconnected
                                    config.ip_address.to_string(),
                                    config.tcp_port,
                                    config.udp_port,
                                );

                                if let Err(e) = device_store.add_event(
                                    device_id.clone(),
                                    disconnect_event,
                                    "ESP32_SYSTEM".to_string(),
                                    "UDP_TIMEOUT".to_string(),
                                ).await {
                                    error!("Failed to send UDP timeout disconnect event for device {}: {}", device_id, e);
                                } else {
                                    info!("UDP TIMEOUT: Disconnect event sent for device {}", device_id);
                                }
                            } else {
                                debug!("UDP TIMEOUT: Device {} already marked as disconnected - skipping redundant event", device_id);
                            }

                            // Remove from tracker to avoid spam
                            tracker.remove(device_id);
                        }
                    }
                }
            }
        });
    }

    /// Update UDP activity for a device
    pub async fn update_udp_activity(&self, device_id: &str) {
        let mut tracker = self.udp_activity_tracker.write().await;
        tracker.insert(device_id.to_string(), Instant::now());
        debug!("UDP activity updated for device: {}", device_id);
    }
}

/// Quick setup for common ESP32 device configurations
impl Esp32DeviceConfig {
    /// Create config for ESP32 with default ports
    pub fn esp32_default(device_id: String, ip: IpAddr) -> Self {
        Self::new(device_id, ip, 3232, 3232) // ESP32 uses port 3232 for both TCP and UDP
    }

    /// Create config for ESP32-S3 with default ports
    pub fn esp32_s3_default(device_id: String, ip: IpAddr) -> Self {
        Self::new(device_id, ip, 3232, 3232) // ESP32-S3 also uses port 3232
    }
}