// ESP32 TCP/UDP connection management

use crate::esp32_types::{
    Esp32Command, Esp32Event, Esp32DeviceConfig, ConnectionState, Esp32Result, Esp32Error, Esp32Variable
};

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};
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
    udp_socket: Arc<Mutex<Option<UdpSocket>>>,
    connection_state: Arc<RwLock<ConnectionState>>,
    event_sender: mpsc::UnboundedSender<Esp32Event>,
    tcp_buffer: Arc<Mutex<String>>,
    shutdown_sender: Option<mpsc::UnboundedSender<()>>,
}

impl Esp32Connection {
    /// Create a new ESP32 connection manager
    pub fn new(config: Esp32DeviceConfig, event_sender: mpsc::UnboundedSender<Esp32Event>) -> Self {
        Self {
            config,
            tcp_stream: Arc::new(Mutex::new(None)),
            udp_socket: Arc::new(Mutex::new(None)),
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
        
        // Setup UDP listener first
        self.setup_udp_listener().await?;
        
        // Then establish TCP connection
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
        let _ = self.event_sender.send(event);
        
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
        
        {
            let mut udp = self.udp_socket.lock().await;
            *udp = None;
        }
        
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
        let _ = self.event_sender.send(event);
        
        info!("Disconnected from ESP32 device {}", self.config.device_id);
        Ok(())
    }
    
    /// Send command to ESP32 via TCP
    pub async fn send_command(&self, command: Esp32Command) -> Esp32Result<()> {
        debug!("Sending command to ESP32 {}: {:?}", self.config.device_id, command);
        
        let json_str = command.to_json()?;
        
        let mut tcp = self.tcp_stream.lock().await;
        if let Some(stream) = tcp.as_mut() {
            stream.write_all(json_str.as_bytes()).await?;
            stream.flush().await?;
            debug!("Command sent successfully: {}", json_str);
            Ok(())
        } else {
            Err(Esp32Error::ConnectionFailed("No TCP connection available".to_string()))
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
        let event_sender = self.event_sender.clone();
        let connection_state = Arc::clone(&self.connection_state);
        let device_id = self.config.device_id.clone();
        
        tokio::spawn(async move {
            let mut buffer = [0u8; 1024];
            
            loop {
                // Check for shutdown signal
                if shutdown_rx.try_recv().is_ok() {
                    debug!("TCP listener task shutting down for device {}", device_id);
                    break;
                }
                
                // Check if we have a TCP connection
                let mut tcp = tcp_stream.lock().await;
                if let Some(stream) = tcp.as_mut() {
                    match timeout(Duration::from_millis(100), stream.read(&mut buffer)).await {
                        Ok(Ok(0)) => {
                            // Connection closed
                            warn!("TCP connection closed by ESP32 device {}", device_id);
                            *tcp = None;
                            
                            let mut state = connection_state.write().await;
                            *state = ConnectionState::Disconnected;
                            break;
                        }
                        Ok(Ok(bytes_read)) => {
                            // Data received
                            let chunk = String::from_utf8_lossy(&buffer[..bytes_read]);
                            debug!("Received TCP data from {}: {}", device_id, chunk);
                            
                            // Append to buffer
                            {
                                let mut buf = tcp_buffer.lock().await;
                                buf.push_str(&chunk);
                                
                                // Try to extract complete JSON messages
                                while let Some(json_str) = extract_complete_json(&mut buf) {
                                    if let Err(e) = handle_tcp_message(&json_str, &event_sender).await {
                                        error!("Failed to handle TCP message from {}: {}", device_id, e);
                                    }
                                }
                            }
                        }
                        Ok(Err(e)) => {
                            error!("TCP read error for device {}: {}", device_id, e);
                            *tcp = None;
                            
                            let mut state = connection_state.write().await;
                            *state = ConnectionState::Failed(e.to_string());
                            break;
                        }
                        Err(_) => {
                            // Timeout, continue loop
                        }
                    }
                } else {
                    // No connection, wait a bit
                    sleep(Duration::from_millis(100)).await;
                }
            }
            
            debug!("TCP listener task ended for device {}", device_id);
        });
    }
    
    // ========================================================================
    // UDP LISTENER SETUP
    // ========================================================================
    
    /// Setup UDP socket for receiving broadcasts from ESP32
    async fn setup_udp_listener(&self) -> Esp32Result<()> {
        let udp_addr = SocketAddr::new(self.config.ip_address, self.config.udp_port);
        debug!("Setting up UDP listener on {}", udp_addr);
        
        let socket = UdpSocket::bind(udp_addr)
            .await
            .map_err(|e| Esp32Error::ConnectionFailed(format!("UDP bind failed: {}", e)))?;
        
        {
            let mut udp = self.udp_socket.lock().await;
            *udp = Some(socket);
        }
        
        // Start UDP listener task
        self.start_udp_listener_task().await;
        
        debug!("UDP listener setup complete on {}", udp_addr);
        Ok(())
    }
    
    /// Start background task for UDP message handling
    async fn start_udp_listener_task(&self) {
        let udp_socket = Arc::clone(&self.udp_socket);
        let event_sender = self.event_sender.clone();
        let device_id = self.config.device_id.clone();
        
        tokio::spawn(async move {
            let mut buffer = [0u8; 1024];
            
            loop {
                let udp = udp_socket.lock().await;
                if let Some(socket) = udp.as_ref() {
                    match timeout(Duration::from_millis(100), socket.recv_from(&mut buffer)).await {
                        Ok(Ok((bytes_read, from_addr))) => {
                            let message = String::from_utf8_lossy(&buffer[..bytes_read]).to_string();
                            debug!("Received UDP broadcast from {}: {}", from_addr, message);
                            
                            // Parse for variable updates
                            handle_udp_message(&message, from_addr, &event_sender).await;
                        }
                        Ok(Err(e)) => {
                            error!("UDP receive error for device {}: {}", device_id, e);
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
async fn handle_tcp_message(json_str: &str, event_sender: &mpsc::UnboundedSender<Esp32Event>) -> Esp32Result<()> {
    let value: Value = serde_json::from_str(json_str)?;
    
    // Handle changeableVariables array
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
                let event = Esp32Event::changeable_variables(variables);
                let _ = event_sender.send(event);
            }
        }
    }
    
    // Handle startOptions array
    if let Some(options_array) = value.get("startOptions") {
        if let Some(options) = options_array.as_array() {
            let mut start_options = Vec::new();
            for option in options {
                if let Some(option_str) = option.as_str() {
                    start_options.push(option_str.to_string());
                }
            }
            
            if !start_options.is_empty() {
                let event = Esp32Event::start_options(start_options);
                let _ = event_sender.send(event);
            }
        }
    }
    
    Ok(())
}

/// Handle incoming UDP message from ESP32
async fn handle_udp_message(message: &str, from_addr: SocketAddr, event_sender: &mpsc::UnboundedSender<Esp32Event>) {
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