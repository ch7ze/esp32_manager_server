// ESP32 device manager - handles multiple ESP32 connections and integrates with device store

use crate::esp32_connection::{Esp32Connection};
use crate::esp32_types::{
    Esp32Command, Esp32Event, Esp32DeviceConfig, ConnectionState, Esp32Result, Esp32Error
};
use crate::device_store::{SharedDeviceStore, DeviceEventStore};
use crate::events::DeviceEvent;
use crate::debug_logger::DebugLogger;

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
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
    /// Device configurations
    device_configs: Arc<RwLock<HashMap<String, Esp32DeviceConfig>>>,
    /// Shared device store for event management
    device_store: SharedDeviceStore,
    /// Central UDP listener for all ESP32 devices
    central_udp_socket: Arc<Mutex<Option<UdpSocket>>>,
    /// Map of IP -> device_id for UDP message routing
    ip_to_device_id: Arc<RwLock<HashMap<IpAddr, String>>>,
    /// Global mutex to prevent race conditions during device connections
    connection_mutex: Arc<Mutex<()>>,
    /// UDP activity tracking for device connectivity monitoring
    udp_activity_tracker: Arc<RwLock<HashMap<String, Instant>>>,
    /// Connection state tracking to prevent redundant events (device_id -> is_connected)
    udp_connection_states: Arc<RwLock<HashMap<String, bool>>>,
}


impl Esp32Manager {
    /// Create new ESP32 manager
    pub fn new(device_store: SharedDeviceStore) -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            device_configs: Arc::new(RwLock::new(HashMap::new())),
            device_store,
            central_udp_socket: Arc::new(Mutex::new(None)),
            ip_to_device_id: Arc::new(RwLock::new(HashMap::new())),
            connection_mutex: Arc::new(Mutex::new(())),
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
        let connection = Esp32Connection::new(config, device_event_sender, self.device_store.clone());

        {
            let mut connections = self.connections.write().await;
            crate::debug_logger::DebugLogger::log_device_manager_state(&device_id, "ADDING to connections HashMap");
            connections.insert(device_id.clone(), Arc::new(Mutex::new(connection)));
            crate::debug_logger::DebugLogger::log_event("ESP32_MANAGER", &format!("CONNECTION_STORED_IN_HASHMAP: {}", device_id));
            crate::debug_logger::DebugLogger::log_device_manager_state(&device_id, "ADDED to connections HashMap");
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
            crate::debug_logger::DebugLogger::log_device_manager_state(device_id, "REMOVING from connections HashMap");
            connections.remove(device_id);
            crate::debug_logger::DebugLogger::log_device_manager_state(device_id, "REMOVED from connections HashMap");
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
                        info!("DEVICE CONNECTION DEBUG: Device {} is in connecting state (likely after reset) - attempting reconnect", device_id);
                        crate::debug_logger::DebugLogger::log_event("ESP32_MANAGER", &format!("CONNECTING_STATE_RECONNECT: {}", device_id));
                        false // Use existing connection and try to reconnect
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
            let new_connection = Esp32Connection::new(config.clone(), direct_sender, self.device_store.clone());
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
    

    /// Create a direct device event sender - SIMPLIFIED VERSION
    /// This sends events directly to the DeviceStore, bypassing all intermediate processing
    fn create_direct_device_sender(&self, device_id: String) -> mpsc::UnboundedSender<Esp32Event> {
        info!("Creating direct device sender for {}", device_id);

        // Create a simple channel that sends events directly to DeviceStore
        let (tx, mut rx) = mpsc::unbounded_channel();
        let device_store = self.device_store.clone();

        // Spawn a simple forwarding task that sends directly to DeviceStore
        tokio::spawn(async move {
            info!("DIRECT SENDER: Started direct forwarding task for device {}", device_id);

            while let Some(esp32_event) = rx.recv().await {
                // Convert ESP32 event to DeviceEvent and send directly to DeviceStore
                if let Err(e) = Self::handle_esp32_event(&device_store, &device_id, esp32_event).await {
                    warn!("DIRECT SENDER: Failed to handle event for device {}: {}", device_id, e);
                }
            }

            info!("DIRECT SENDER: Direct forwarding task ended for device {}", device_id);
        });

        tx
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

                            // Print to terminal only (no logging)
                            println!("UDP Message from {}: {}", from_addr, message);

                            // Route message to specific ESP32 connection if registered
                            {
                                let device_map = ip_to_device_id.read().await;
                                if let Some(device_id) = device_map.get(&from_addr.ip()) {
                                    // Update UDP activity tracking
                                    {
                                        let mut tracker = udp_activity_tracker.write().await;
                                        tracker.insert(device_id.clone(), Instant::now());
                                    }

                                    // Use direct device store bypass - much more reliable than event processor
                                    Self::handle_udp_message_bypass_smart(&message, from_addr, device_id, &device_store, &udp_connection_states).await;
                                } else {
                                    drop(device_map); // Drop read lock before getting write lock

                                    // Check if this looks like a TCP message that should be routed via UDP bypass
                                    if Self::is_tcp_message(&message) {
                                        if let Some(device_id) = Self::extract_device_id_from_tcp_message(&message) {
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
                                        }
                                    }
                                    // No logging for unregistered devices or non-TCP messages
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

    /// Update device configuration if UDP port has changed (after ESP32 restart)
    /// Note: This is now a placeholder since global config access was removed
    async fn update_device_config_for_udp_port_change(device_id: &str, from_addr: SocketAddr) {
        debug!("UDP port check for device {} on port {} - global config access removed", device_id, from_addr.port());
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

        // Update device configuration if UDP port has changed (important for TCP connections after ESP32 restart)
        Self::update_device_config_for_udp_port_change(device_id, from_addr).await;

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
        DebugLogger::log_tcp_message(device_id, "RECEIVED", message);

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
                let variable_event = crate::events::DeviceEvent::esp32_variable_update(
                    device_id.to_string(),
                    name.as_str().trim().to_string(),
                    value.as_str().trim().to_string(),
                );
                let _ = device_store.add_event(device_id.to_string(), variable_event, "esp32_system".to_string(), "tcp_bypass".to_string()).await;
            }
        }
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
    let manager = Arc::new(Esp32Manager::new(device_store));


    manager
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