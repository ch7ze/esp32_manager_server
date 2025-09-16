// ESP32 device manager - handles multiple ESP32 connections and integrates with device store

use crate::esp32_connection::{Esp32Connection, handle_udp_message};
use crate::esp32_types::{
    Esp32Command, Esp32Event, Esp32DeviceConfig, ConnectionState, Esp32Result, Esp32Error
};
use crate::device_store::{SharedDeviceStore, DeviceEventStore};
use crate::events::DeviceEvent;

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock, Mutex};
use tokio::net::UdpSocket;
use tokio::time::{sleep, timeout};
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
    /// Event sender for internal communication
    event_sender: mpsc::UnboundedSender<Esp32ManagerEvent>,
    /// Event receiver for processing
    event_receiver: Arc<Mutex<mpsc::UnboundedReceiver<Esp32ManagerEvent>>>,
    /// Central UDP listener for all ESP32 devices
    central_udp_socket: Arc<Mutex<Option<UdpSocket>>>,
    /// Map of IP -> device_id for UDP message routing
    ip_to_device_id: Arc<RwLock<HashMap<IpAddr, String>>>,
}

/// Internal events for ESP32 manager
#[derive(Debug)]
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
            device_configs: Arc::new(RwLock::new(HashMap::new())),
            device_store,
            event_sender,
            event_receiver: Arc::new(Mutex::new(event_receiver)),
            central_udp_socket: Arc::new(Mutex::new(None)),
            ip_to_device_id: Arc::new(RwLock::new(HashMap::new())),
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

        info!("ESP32 Manager started");
    }
    
    /// Add a new ESP32 device configuration
    pub async fn add_device(&self, config: Esp32DeviceConfig) -> Esp32Result<()> {
        let device_id = config.device_id.clone();
        info!("Adding ESP32 device: {} ({}:{})", 
               device_id, config.ip_address, config.tcp_port);
        
        // Store configuration
        {
            let mut configs = self.device_configs.write().await;
            configs.insert(device_id.clone(), config.clone());
        }
        
        // Create connection but don't connect yet
        let event_tx = self.create_device_event_sender(device_id.clone());
        let connection = Esp32Connection::new(config, event_tx);
        
        {
            let mut connections = self.connections.write().await;
            connections.insert(device_id.clone(), Arc::new(Mutex::new(connection)));
        }
        
        info!("ESP32 device {} added successfully", device_id);
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
            let mut configs = self.device_configs.write().await;
            configs.remove(device_id);
        }
        
        info!("ESP32 device {} removed", device_id);
        Ok(())
    }
    
    /// Connect to ESP32 device
    pub async fn connect_device(&self, device_id: &str) -> Esp32Result<()> {
        info!("Connecting to ESP32 device: {}", device_id);

        let connections = self.connections.read().await;
        if let Some(connection_arc) = connections.get(device_id) {
            let mut connection = connection_arc.lock().await;
            connection.connect().await?;

            // Register device for central UDP routing
            let config = {
                let configs = self.device_configs.read().await;
                configs.get(device_id).cloned()
            };

            if let Some(config) = config {
                self.register_esp32_for_udp(device_id.to_string(), config.ip_address).await;
            }

            info!("Successfully connected to ESP32 device: {}", device_id);
            Ok(())
        } else {
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
        
        let (tx, mut rx) = mpsc::unbounded_channel();
        
        // Forward device events to manager
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                let _ = manager_sender.send(Esp32ManagerEvent::DeviceEvent(device_id.clone(), event));
            }
        });
        
        tx
    }
    
    /// Start background event processor
    async fn start_event_processor(&self) {
        let event_receiver = Arc::clone(&self.event_receiver);
        let device_store = Arc::clone(&self.device_store);
        
        tokio::spawn(async move {
            let mut receiver = event_receiver.lock().await;
            
            while let Some(event) = receiver.recv().await {
                match event {
                    Esp32ManagerEvent::DeviceEvent(device_id, esp32_event) => {
                        if let Err(e) = Self::handle_esp32_event(&device_store, &device_id, esp32_event).await {
                            error!("Failed to handle ESP32 event for device {}: {}", device_id, e);
                        }
                    }
                    Esp32ManagerEvent::ConnectionStateChanged(device_id, state) => {
                        info!("ESP32 device {} connection state changed: {:?}", device_id, state);
                        // TODO: Notify connected clients about state change
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
        
        // Convert ESP32 event to DeviceEvent
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
                DeviceEvent::esp32_connection_status(device_id.to_string(), connected, device_ip, tcp_port, udp_port)
            }
            Esp32Event::DeviceInfo { device_id: _, device_name, firmware_version, uptime } => {
                DeviceEvent::esp32_device_info(device_id.to_string(), device_name, firmware_version, uptime)
            }
        };
        
        // Add event to device store (this will broadcast to all connected WebSocket clients)
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
        let connections = Arc::clone(&self.connections);

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
                            let device_map = ip_to_device_id.read().await;
                            if let Some(device_id) = device_map.get(&from_addr.ip()) {
                                let connections_guard = connections.read().await;
                                if let Some(connection) = connections_guard.get(device_id) {
                                    let conn = connection.lock().await;
                                    if let Some(sender) = conn.get_event_sender() {
                                        // Process UDP message using existing handler
                                        handle_udp_message(&message, from_addr, sender).await;
                                        debug!("UDP message processed for device {}", device_id);
                                    }
                                } else {
                                    debug!("No connection found for device {}", device_id);
                                }
                            } else {
                                debug!("No device registered for IP {}", from_addr.ip());
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

/// Quick setup for common ESP32 device configurations
impl Esp32DeviceConfig {
    /// Create config for ESP32 with default ports
    pub fn esp32_default(device_id: String, ip: IpAddr) -> Self {
        Self::new(device_id, ip, 23, 1234) // Common ESP32 ports
    }
    
    /// Create config for ESP32-S3 with default ports  
    pub fn esp32_s3_default(device_id: String, ip: IpAddr) -> Self {
        Self::new(device_id, ip, 23, 1235) // Different UDP port for S3
    }
}