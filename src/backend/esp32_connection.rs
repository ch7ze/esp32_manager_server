// ESP32 TCP/UDP connection management

use crate::esp32_types::{
    Esp32Command, Esp32Event, Esp32DeviceConfig, ConnectionState, Esp32Result, Esp32Error, Esp32Variable
};

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, RwLock, Mutex};
use tokio::time::{timeout, sleep};
use tracing::{info, warn, error, debug};
use serde_json::Value;

// ============================================================================
// ESP32 CONNECTION MANAGER
// ============================================================================

#[derive(Debug)]
pub struct Esp32Connection {
    config: Esp32DeviceConfig,
    tcp_stream: Arc<Mutex<Option<TcpStream>>>,
    connection_state: Arc<RwLock<ConnectionState>>,
    event_sender: mpsc::UnboundedSender<Esp32Event>,
    tcp_buffer: Arc<Mutex<String>>,
    shutdown_sender: Option<mpsc::UnboundedSender<()>>,
}

impl Esp32Connection {
    /// Create a new ESP32 connection manager
    pub fn new(config: Esp32DeviceConfig, event_sender: mpsc::UnboundedSender<Esp32Event>) -> Self {
        info!("ESP32CONNECTION CREATION DEBUG: Creating new ESP32Connection for device {}", config.device_id);
        crate::debug_logger::DebugLogger::log_event("ESP32_CONNECTION", &format!("NEW_CONNECTION_CREATED: {} - sender_closed: {}", config.device_id, event_sender.is_closed()));

        Self {
            config,
            tcp_stream: Arc::new(Mutex::new(None)),
            connection_state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            event_sender,
            tcp_buffer: Arc::new(Mutex::new(String::new())),
            shutdown_sender: None,
        }
    }
    
    /// Get current connection state
    pub async fn get_connection_state(&self) -> ConnectionState {
        self.connection_state.read().await.clone()
    }
    
    /// Start connection to ESP32 (both TCP and UDP)
    pub async fn connect(&mut self) -> Esp32Result<()> {
        info!("Connecting to ESP32 device {} at {}", 
               self.config.device_id, self.config.ip_address);
        
        // Set connecting state
        {
            let mut state = self.connection_state.write().await;
            *state = ConnectionState::Connecting;
        }
        
        // Establish TCP connection (UDP is now handled centrally)
        // No individual UDP listener needed anymore
        self.connect_tcp().await?;
        
        // Start background tasks
        let (shutdown_tx, shutdown_rx) = mpsc::unbounded_channel();
        self.shutdown_sender = Some(shutdown_tx);
        
        // Start TCP listener task
        self.start_tcp_listener_task(shutdown_rx).await;
        
        // Send connection status event
        let event = Esp32Event::connection_status(
            true,
            self.config.ip_address,
            self.config.tcp_port,
            self.config.udp_port
        );
        info!("ESP32CONNECTION DEBUG: About to send connection status event (connected=true) for device {}", self.config.device_id);
        info!("ESP32CONNECTION DEBUG: Event sender channel status - is_closed: {}", self.event_sender.is_closed());
        crate::debug_logger::DebugLogger::log_event("ESP32_CONNECTION", &format!("ABOUT_TO_SEND_CONNECTION_STATUS: {} - sender_closed: {}", self.config.device_id, self.event_sender.is_closed()));

        let is_closed = self.event_sender.is_closed();
        if is_closed {
            warn!("ESP32CONNECTION DEBUG: Event sender is closed for device {}, connection status event will be skipped", self.config.device_id);
            warn!("ESP32CONNECTION DEBUG: This explains why frontend shows 'Disconnected' - event channel is closed!");
            warn!("ESP32CONNECTION DEBUG: The ESP32 is actually connected via TCP, but status events cannot be sent to frontend");
            crate::debug_logger::DebugLogger::log_esp32_connection_event_send(&self.config.device_id, is_closed, false, Some("Event sender is closed"));
        } else {
            match self.event_sender.send(event) {
                Ok(()) => {
                    info!("ESP32CONNECTION DEBUG: Connection status event sent successfully for device {}", self.config.device_id);
                    info!("ESP32CONNECTION DEBUG: Event should now flow: ESP32Connection -> EventForwardingTask -> ESP32Manager -> DeviceStore -> WebSocket -> Frontend");
                    crate::debug_logger::DebugLogger::log_esp32_connection_event_send(&self.config.device_id, is_closed, true, None);
                }
                Err(e) => {
                    error!("ESP32CONNECTION DEBUG: FAILED to send connection status event for device {}: {}", self.config.device_id, e);
                    error!("ESP32CONNECTION DEBUG: Event sender is_closed: {}", self.event_sender.is_closed());
                    error!("ESP32CONNECTION DEBUG: This means the Event Forwarding Task receiver has been dropped!");
                    error!("ESP32CONNECTION DEBUG: This explains why frontend shows 'Disconnected' - event channel is closed!");
                    crate::debug_logger::DebugLogger::log_esp32_connection_event_send(&self.config.device_id, is_closed, false, Some(&e.to_string()));
                }
            }
        }
        
        info!("Successfully connected to ESP32 device {}", self.config.device_id);
        Ok(())
    }
    
    /// Disconnect from ESP32
    pub async fn disconnect(&mut self) -> Esp32Result<()> {
        info!("Disconnecting from ESP32 device {}", self.config.device_id);
        
        // Send shutdown signal
        if let Some(shutdown_tx) = &self.shutdown_sender {
            let _ = shutdown_tx.send(());
        }
        
        // Close connections
        {
            let mut tcp = self.tcp_stream.lock().await;
            if let Some(mut stream) = tcp.take() {
                let _ = stream.shutdown().await;
            }
        }
        
        // UDP is now handled centrally
        
        // Update state
        {
            let mut state = self.connection_state.write().await;
            *state = ConnectionState::Disconnected;
        }
        
        // Send connection status event
        let event = Esp32Event::connection_status(
            false,
            self.config.ip_address,
            self.config.tcp_port,
            self.config.udp_port
        );
        info!("Sending connection status event (connected=false) for device {}", self.config.device_id);
        if let Err(e) = self.event_sender.send(event) {
            error!("Failed to send disconnect status event for device {}: {}", self.config.device_id, e);
        } else {
            info!("Disconnect status event sent successfully for device {}", self.config.device_id);
        }
        
        info!("Disconnected from ESP32 device {}", self.config.device_id);
        Ok(())
    }
    
    /// Send command to ESP32 via TCP
    pub async fn send_command(&self, command: Esp32Command) -> Esp32Result<()> {
        debug!("Sending command to ESP32 {}: {:?}", self.config.device_id, command);

        // Check if this is a reset command (which will close the TCP connection)
        let is_reset_command = matches!(command, Esp32Command::Reset { .. });
        if is_reset_command {
            info!("RESET COMMAND: ESP32 {} will reset and close TCP connection - this is expected behavior", self.config.device_id);
        }

        let json_str = command.to_json()?;

        let mut tcp = self.tcp_stream.lock().await;
        if let Some(stream) = tcp.as_mut() {
            // Send the command
            let write_result = stream.write_all(json_str.as_bytes()).await;
            if let Err(e) = write_result {
                if is_reset_command {
                    info!("RESET COMMAND: Write failed for device {} (expected during reset): {}", self.config.device_id, e);
                    return Ok(()); // Reset commands are expected to fail during write/flush
                } else {
                    return Err(e.into());
                }
            }

            // Flush the command
            let flush_result = stream.flush().await;
            if let Err(e) = flush_result {
                if is_reset_command {
                    info!("RESET COMMAND: Flush failed for device {} (expected during reset): {}", self.config.device_id, e);
                    return Ok(()); // Reset commands are expected to fail during write/flush
                } else {
                    return Err(e.into());
                }
            }

            debug!("Command sent successfully: {}", json_str);

            // For reset commands, immediately mark connection as disconnected
            if is_reset_command {
                info!("RESET COMMAND: Marking connection as disconnected for device {} after reset", self.config.device_id);
                *tcp = None; // Close our side of the connection

                // Update connection state
                {
                    let mut state = self.connection_state.write().await;
                    *state = ConnectionState::Disconnected;
                }

                // Send disconnect event
                if let Err(e) = crate::esp32_manager::handle_tcp_disconnect_global(&self.config.device_id).await {
                    warn!("RESET COMMAND: Failed to send disconnect event after reset for device {}: {}", self.config.device_id, e);
                } else {
                    info!("RESET COMMAND: Disconnect event sent after reset for device {}", self.config.device_id);
                }
            }

            Ok(())
        } else {
            // No TCP connection available, try to reconnect
            debug!("No TCP connection available for device {}, attempting reconnection", self.config.device_id);
            drop(tcp); // Release the lock before reconnecting

            // Attempt to reconnect
            self.connect_tcp().await?;

            // Send connection status event to notify clients
            let event = Esp32Event::connection_status(
                true,
                self.config.ip_address,
                self.config.tcp_port,
                self.config.udp_port
            );
            if let Err(e) = self.event_sender.send(event) {
                warn!("Failed to send reconnection status event for device {}: {}", self.config.device_id, e);
            } else {
                debug!("Reconnection status event sent for device {}", self.config.device_id);
            }

            // Try sending the command again with the new connection
            let mut tcp = self.tcp_stream.lock().await;
            if let Some(stream) = tcp.as_mut() {
                stream.write_all(json_str.as_bytes()).await?;
                stream.flush().await?;
                debug!("Command sent successfully after reconnection: {}", json_str);
                Ok(())
            } else {
                Err(Esp32Error::ConnectionFailed("Failed to reconnect to ESP32".to_string()))
            }
        }
    }
    
    // ========================================================================
    // TCP CONNECTION HANDLING
    // ========================================================================
    
    /// Establish TCP connection to ESP32
    async fn connect_tcp(&self) -> Esp32Result<()> {
        let tcp_addr = self.config.tcp_addr();
        debug!("Connecting to TCP address: {}", tcp_addr);

        // Try to connect with timeout
        let stream = timeout(Duration::from_secs(5), TcpStream::connect(tcp_addr))
            .await
            .map_err(|_| Esp32Error::Timeout)?
            .map_err(|e| Esp32Error::ConnectionFailed(format!("TCP connection failed: {}", e)))?;

        // Configure TCP socket for faster disconnect detection
        if let Err(e) = stream.set_nodelay(true) {
            warn!("Failed to set TCP_NODELAY for device {}: {}", self.config.device_id, e);
        }

        // Enable TCP keep-alive with shorter intervals
        let socket2_socket = socket2::Socket::from(stream.into_std()?);

        // Enable keep-alive
        if let Err(e) = socket2_socket.set_keepalive(true) {
            warn!("Failed to enable TCP keep-alive for device {}: {}", self.config.device_id, e);
        }

        // Set reasonable TCP keep-alive for disconnect detection
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        {
            use socket2::TcpKeepalive;
            let keepalive = TcpKeepalive::new()
                .with_time(Duration::from_secs(3))     // Start after 3 seconds of inactivity
                .with_interval(Duration::from_secs(1)); // Send probe every 1 second

            if let Err(e) = socket2_socket.set_tcp_keepalive(&keepalive) {
                warn!("Failed to set TCP keep-alive parameters for device {}: {}", self.config.device_id, e);
            } else {
                info!("TCP keep-alive enabled for device {} (3s idle, 1s interval)", self.config.device_id);
            }
        }

        // Note: Additional Windows TCP optimizations would require more complex winapi setup

        // Note: SO_LINGER removed - it was causing connection issues

        // Convert back to tokio TcpStream
        let stream = TcpStream::from_std(socket2_socket.into())?;
        
        // Store stream
        {
            let mut tcp = self.tcp_stream.lock().await;
            *tcp = Some(stream);
        }
        
        // Update connection state
        {
            let mut state = self.connection_state.write().await;
            *state = ConnectionState::Connected;
        }
        
        debug!("TCP connection established to {}", tcp_addr);
        Ok(())
    }
    
    /// Start background task for TCP message handling
    async fn start_tcp_listener_task(&self, mut shutdown_rx: mpsc::UnboundedReceiver<()>) {
        let tcp_stream = Arc::clone(&self.tcp_stream);
        let tcp_buffer = Arc::clone(&self.tcp_buffer);
        let _event_sender = self.event_sender.clone();
        let connection_state = Arc::clone(&self.connection_state);
        let device_id = self.config.device_id.clone();
        let _device_config = self.config.clone();

        tokio::spawn(async move {
            let mut buffer = [0u8; 1024];
            let mut last_activity = std::time::Instant::now();
            let connection_timeout = Duration::from_secs(5); // 5 seconds without activity = disconnect

            info!("TCP LISTENER TASK: Started for device {} with 5s activity timeout", device_id);

            loop {

                // Check for shutdown signal
                if shutdown_rx.try_recv().is_ok() {
                    debug!("TCP listener task shutting down for device {}", device_id);
                    break;
                }

                // Check if we have a TCP connection for reading
                let mut tcp = tcp_stream.lock().await;
                if let Some(stream) = tcp.as_mut() {
                    match timeout(Duration::from_millis(50), stream.read(&mut buffer)).await {
                        Ok(Ok(0)) => {
                            // Connection closed
                            warn!("TCP connection closed by ESP32 device {}", device_id);
                            *tcp = None;

                            let mut state = connection_state.write().await;
                            *state = ConnectionState::Disconnected;

                            // Send disconnect event to frontend
                            warn!("TCP DISCONNECT: Sending disconnect event to frontend for device {}", device_id);
                            if let Err(e) = crate::esp32_manager::handle_tcp_disconnect_global(&device_id).await {
                                error!("TCP DISCONNECT: Failed to send disconnect event for device {}: {}", device_id, e);
                            } else {
                                info!("TCP DISCONNECT: Disconnect event sent successfully for device {}", device_id);
                            }

                            break;
                        }
                        Ok(Ok(bytes_read)) => {
                            // Data received - reset activity timer
                            last_activity = std::time::Instant::now();

                            let chunk = String::from_utf8_lossy(&buffer[..bytes_read]);
                            debug!("TCP DATA RECEIVED: Device {} sent {} bytes", device_id, bytes_read);
                            debug!("Received TCP data from {}: {}", device_id, chunk);

                            // Append to buffer
                            {
                                let mut buf = tcp_buffer.lock().await;
                                buf.push_str(&chunk);

                                // Try to extract complete JSON messages
                                while let Some(json_str) = extract_complete_json(&mut buf) {
                                    info!("TCP BYPASS: Processing message for device {}: {}", device_id, json_str);

                                    // DISABLE EventForwardingTask for TCP to prevent duplicate events
                                    // EventForwardingTask is unreliable and causes race conditions with TCP bypass
                                    // if let Err(e) = handle_tcp_message(&json_str, &event_sender).await {
                                    //     error!("TCP EventForwardingTask failed for device {}: {}", device_id, e);
                                    // }

                                    // Use ONLY TCP bypass for reliable message processing (like UDP bypass)
                                    info!("TCP BYPASS: Using direct bypass for device {} (EventForwardingTask disabled)", device_id);

                                    // Call global TCP bypass function
                                    if let Err(e) = crate::esp32_manager::handle_tcp_bypass_global(&json_str, &device_id).await {
                                        error!("TCP BYPASS: Global bypass failed for device {}: {}", device_id, e);
                                    }
                                }
                            }
                        }
                        Ok(Err(e)) => {
                            error!("TCP read error for device {}: {}", device_id, e);
                            *tcp = None;

                            let mut state = connection_state.write().await;
                            *state = ConnectionState::Failed(e.to_string());

                            // Send disconnect event to frontend for TCP errors too
                            warn!("TCP ERROR: Sending disconnect event to frontend for device {} due to TCP error: {}", device_id, e);
                            if let Err(disconnect_err) = crate::esp32_manager::handle_tcp_disconnect_global(&device_id).await {
                                error!("TCP ERROR: Failed to send disconnect event for device {}: {}", device_id, disconnect_err);
                            } else {
                                info!("TCP ERROR: Disconnect event sent successfully for device {}", device_id);
                            }

                            break;
                        }
                        Err(_) => {
                            // Read timeout - check if connection is still alive
                            if last_activity.elapsed() > connection_timeout {
                                warn!("TCP ACTIVITY TIMEOUT: No activity for {}s on device {}, assuming disconnected",
                                      connection_timeout.as_secs(), device_id);
                                *tcp = None;

                                let mut state = connection_state.write().await;
                                *state = ConnectionState::Disconnected;

                                // Send disconnect event to frontend
                                warn!("TCP ACTIVITY TIMEOUT: Sending disconnect event to frontend for device {}", device_id);
                                if let Err(e) = crate::esp32_manager::handle_tcp_disconnect_global(&device_id).await {
                                    error!("TCP ACTIVITY TIMEOUT: Failed to send disconnect event for device {}: {}", device_id, e);
                                } else {
                                    info!("TCP ACTIVITY TIMEOUT: Disconnect event sent successfully for device {}", device_id);
                                }

                                break;
                            }
                        }
                    }
                } else {
                    // No connection, wait a bit
                    sleep(Duration::from_millis(100)).await;
                }
            }

            // Connection lost - send disconnect event
            info!("TCP PROBE DISCONNECT: Connection lost for device {}, sending disconnect event", device_id);

            // Update connection state
            {
                let mut state = connection_state.write().await;
                *state = ConnectionState::Disconnected;
            }

            // Send disconnect event to frontend
            if let Err(e) = crate::esp32_manager::handle_tcp_disconnect_global(&device_id).await {
                error!("TCP PROBE DISCONNECT: Failed to send disconnect event for device {}: {}", device_id, e);
            } else {
                info!("TCP PROBE DISCONNECT: Disconnect event sent successfully for device {}", device_id);
            }

            debug!("TCP listener task ended for device {}", device_id);
        });
    }
    
    // ========================================================================
    // UTILITY METHODS
    // ========================================================================

    /// Get event sender for central UDP routing
    pub fn get_event_sender(&self) -> Option<&mpsc::UnboundedSender<Esp32Event>> {
        Some(&self.event_sender)
    }
}

// ============================================================================
// MESSAGE PARSING HELPERS
// ============================================================================

/// Extract complete JSON object from TCP buffer
fn extract_complete_json(buffer: &mut String) -> Option<String> {
    let text = buffer.trim_start();
    if text.is_empty() {
        return None;
    }
    
    let mut bracket_count = 0;
    let mut in_string = false;
    let mut escape_next = false;
    
    for (i, c) in text.char_indices() {
        if escape_next {
            escape_next = false;
            continue;
        }
        
        match c {
            '\\' if in_string => escape_next = true,
            '"' => in_string = !in_string,
            '{' if !in_string => bracket_count += 1,
            '}' if !in_string => {
                bracket_count -= 1;
                if bracket_count == 0 {
                    // Found complete JSON
                    let json_str = text[..=i].to_string();
                    *buffer = text[i + 1..].to_string();
                    return Some(json_str);
                }
            }
            _ => {}
        }
    }
    
    None
}

/// Handle incoming TCP message from ESP32
/// Enhanced version that matches the UDP bypass functionality and C# implementation
async fn handle_tcp_message(json_str: &str, event_sender: &mpsc::UnboundedSender<Esp32Event>) -> Esp32Result<()> {
    let value: Value = serde_json::from_str(json_str)?;

    // Handle changeableVariables array (from C# RemoteAccess.cs line 347-368)
    if let Some(vars_array) = value.get("changeableVariables") {
        if let Some(vars) = vars_array.as_array() {
            let mut variables = Vec::new();
            for var in vars {
                if let (Some(name), Some(value)) = (var.get("name"), var.get("value")) {
                    if let (Some(name_str), Some(value_num)) = (name.as_str(), value.as_u64()) {
                        variables.push(Esp32Variable {
                            name: name_str.to_string(),
                            value: value_num as u32,
                        });
                    }
                }
            }

            if !variables.is_empty() {
                debug!("TCP: Extracted changeableVariables: {:?}", variables);
                let event = Esp32Event::changeable_variables(variables);
                let _ = event_sender.send(event);
            }
        }
    }

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
                debug!("TCP: Extracted startOptions: {:?}", start_options);
                let event = Esp32Event::start_options(start_options);
                let _ = event_sender.send(event);
            }
        }
    }

    // Handle device information (enhanced from ESP32 capabilities)
    if let Some(device_name) = value.get("deviceName").and_then(|v| v.as_str()) {
        let firmware_version = value.get("firmwareVersion").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
        let uptime = value.get("uptime").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

        debug!("TCP: Extracted device info - name: {}, firmware: {}, uptime: {}", device_name, firmware_version, uptime);
        let device_info_event = Esp32Event::DeviceInfo {
            device_id: "tcp_device".to_string(), // Will be overridden by manager
            device_name: Some(device_name.to_string()),
            firmware_version: Some(firmware_version),
            uptime: Some(uptime as u64),
        };
        let _ = event_sender.send(device_info_event);
    }

    // Handle individual variable updates (similar to UDP parsing)
    for (key, val) in value.as_object().unwrap_or(&serde_json::Map::new()) {
        if key != "changeableVariables" && key != "startOptions" && key != "deviceName" && key != "firmwareVersion" && key != "uptime" {
            if let Some(val_str) = val.as_str() {
                debug!("TCP: Extracted individual variable - {}={}", key, val_str);
                let variable_event = Esp32Event::variable_update(key.clone(), val_str.to_string());
                let _ = event_sender.send(variable_event);
            } else if let Some(val_num) = val.as_u64() {
                debug!("TCP: Extracted individual numeric variable - {}={}", key, val_num);
                let variable_event = Esp32Event::variable_update(key.clone(), val_num.to_string());
                let _ = event_sender.send(variable_event);
            }
        }
    }

    Ok(())
}

/// Handle TCP message using direct bypass to DeviceStore (like UDP bypass)
/// This ensures TCP events reach the frontend even when EventForwardingTask crashes
async fn handle_tcp_message_bypass(json_str: &str, device_id: &str) {
    info!("TCP BYPASS: Processing TCP message for device {}: {}", device_id, json_str);

    // For now, we'll log the bypass logic but can't call DeviceStore directly from here
    // The DeviceStore access needs to be done from ESP32Manager
    // This is a placeholder that will be replaced by a call from ESP32Manager

    warn!("TCP BYPASS: TCP bypass logic needs to be called from ESP32Manager with DeviceStore access");
    warn!("TCP BYPASS: Message for device {}: {}", device_id, json_str);
}

/// Handle incoming UDP message from ESP32
pub async fn handle_udp_message(message: &str, from_addr: SocketAddr, event_sender: &mpsc::UnboundedSender<Esp32Event>) {
    // Console output is now handled by central UDP listener
    debug!("Processing UDP message from {}: {}", from_addr, message);

    // Send raw UDP broadcast event
    let broadcast_event = Esp32Event::udp_broadcast(message.to_string(), from_addr);
    let _ = event_sender.send(broadcast_event);
    
    // Parse for variable updates using regex like the C# version
    let re = regex::Regex::new(r#"\{"([^"]+)"\s*:\s*"([^"]+)"\}"#).unwrap();
    for captures in re.captures_iter(message) {
        if let (Some(name), Some(value)) = (captures.get(1), captures.get(2)) {
            let variable_event = Esp32Event::variable_update(
                name.as_str().trim().to_string(),
                value.as_str().trim().to_string(),
            );
            let _ = event_sender.send(variable_event);
        }
    }
}

impl Drop for Esp32Connection {
    fn drop(&mut self) {
        error!("ESP32CONNECTION DROP DEBUG: ESP32Connection for device {} is being DROPPED! This will close the event_sender!", self.config.device_id);
        crate::debug_logger::DebugLogger::log_event("ESP32_CONNECTION", &format!("CONNECTION_DROPPED: {}", self.config.device_id));

        // Send shutdown signal if we have one
        if let Some(shutdown_tx) = &self.shutdown_sender {
            let _ = shutdown_tx.send(());
        }
    }
}