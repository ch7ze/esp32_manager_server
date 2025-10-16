// UART connection management for ESP32 devices
// Handles serial communication with multiple ESP32 devices connected via UART

use crate::device_store::SharedDeviceStore;
use crate::esp32_manager::Esp32Manager;

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tokio::time::sleep;
use tokio_serial::{SerialPort, SerialPortBuilderExt, SerialStream};
use tracing::{info, warn, error, debug};

// ============================================================================
// UART CONNECTION MANAGER
// ============================================================================

/// UART connection settings
#[derive(Debug, Clone)]
pub struct UartSettings {
    pub port: String,
    pub baud_rate: u32,
}

impl Default for UartSettings {
    fn default() -> Self {
        Self {
            port: String::new(),
            baud_rate: 115200,
        }
    }
}

/// Manages UART connection for ESP32 devices
pub struct UartConnection {
    /// Current UART settings
    settings: Arc<RwLock<Option<UartSettings>>>,
    /// Serial port stream
    serial_stream: Arc<RwLock<Option<SerialStream>>>,
    /// Device store for event routing
    device_store: SharedDeviceStore,
    /// Shutdown channel
    shutdown_sender: Option<mpsc::UnboundedSender<()>>,
    /// Connection status
    is_connected: Arc<RwLock<bool>>,
}

impl UartConnection {
    /// Create new UART connection manager
    pub fn new(device_store: SharedDeviceStore) -> Self {
        Self {
            settings: Arc::new(RwLock::new(None)),
            serial_stream: Arc::new(RwLock::new(None)),
            device_store,
            shutdown_sender: None,
            is_connected: Arc::new(RwLock::new(false)),
        }
    }

    /// Connect to UART port with given settings
    pub async fn connect(&mut self, port: String, baud_rate: u32) -> Result<(), String> {
        info!("Connecting to UART port {} at {} baud", port, baud_rate);

        // Close existing connection if any
        self.disconnect().await?;

        // Try to open serial port
        let serial_stream = tokio_serial::new(&port, baud_rate)
            .timeout(Duration::from_millis(1000))
            .open_native_async()
            .map_err(|e| format!("Failed to open UART port {}: {}", port, e))?;

        info!("UART port {} opened successfully", port);

        // Store settings and stream
        {
            let mut settings = self.settings.write().await;
            *settings = Some(UartSettings {
                port: port.clone(),
                baud_rate,
            });
        }

        {
            let mut stream = self.serial_stream.write().await;
            *stream = Some(serial_stream);
        }

        {
            let mut connected = self.is_connected.write().await;
            *connected = true;
        }

        // Start UART listener task
        let (shutdown_tx, shutdown_rx) = mpsc::unbounded_channel();
        self.shutdown_sender = Some(shutdown_tx);
        self.start_uart_listener_task(shutdown_rx).await;

        info!("UART connection established on port {}", port);
        Ok(())
    }

    /// Disconnect from UART port
    pub async fn disconnect(&mut self) -> Result<(), String> {
        info!("Disconnecting UART connection");

        // Send shutdown signal
        if let Some(shutdown_tx) = &self.shutdown_sender {
            let _ = shutdown_tx.send(());
        }

        // Close serial port
        {
            let mut stream = self.serial_stream.write().await;
            *stream = None;
        }

        {
            let mut connected = self.is_connected.write().await;
            *connected = false;
        }

        info!("UART connection closed");
        Ok(())
    }

    /// Get current connection status
    pub async fn is_connected(&self) -> bool {
        *self.is_connected.read().await
    }

    /// Get current settings
    pub async fn get_settings(&self) -> Option<UartSettings> {
        self.settings.read().await.clone()
    }

    /// Start background task for UART message handling
    async fn start_uart_listener_task(&self, mut shutdown_rx: mpsc::UnboundedReceiver<()>) {
        let serial_stream = Arc::clone(&self.serial_stream);
        let device_store = self.device_store.clone();
        let is_connected = Arc::clone(&self.is_connected);

        tokio::spawn(async move {
            info!("UART listener task started");

            let mut buffer = String::new();
            let mut read_buffer = vec![0u8; 1024];

            loop {
                // Check for shutdown signal
                if shutdown_rx.try_recv().is_ok() {
                    debug!("UART listener task shutting down");
                    break;
                }

                // Read from UART stream
                let mut stream_guard = serial_stream.write().await;
                if let Some(stream) = stream_guard.as_mut() {
                    // Try to read with timeout
                    use tokio::io::AsyncReadExt;

                    let read_result = tokio::time::timeout(
                        Duration::from_millis(100),
                        stream.read(&mut read_buffer)
                    ).await;

                    match read_result {
                        Ok(Ok(0)) => {
                            // Connection closed
                            warn!("UART connection closed");
                            drop(stream_guard);
                            *is_connected.write().await = false;
                            break;
                        }
                        Ok(Ok(bytes_read)) => {
                            // Got data from UART
                            let data = String::from_utf8_lossy(&read_buffer[..bytes_read]);
                            buffer.push_str(&data);

                            // Process complete lines (messages end with \n or \r\n)
                            while let Some(line_end) = buffer.find('\n') {
                                let line = buffer[..line_end].trim().to_string();
                                buffer.drain(..=line_end);

                                if !line.is_empty() {
                                    info!("UART RECEIVED: {}", line);

                                    // Process the message
                                    let device_store_clone = device_store.clone();
                                    let line_clone = line.clone();
                                    tokio::spawn(async move {
                                        Self::handle_uart_message(&line_clone, &device_store_clone).await;
                                    });
                                }
                            }
                        }
                        Ok(Err(e)) => {
                            // Read error
                            error!("UART read error: {}", e);
                            drop(stream_guard);
                            *is_connected.write().await = false;
                            break;
                        }
                        Err(_) => {
                            // Timeout - no data available, continue loop
                        }
                    }
                    drop(stream_guard);
                } else {
                    // No connection, wait a bit
                    sleep(Duration::from_millis(100)).await;
                }
            }

            info!("UART listener task ended");
        });
    }

    /// Handle incoming UART message
    async fn handle_uart_message(message: &str, device_store: &SharedDeviceStore) {
        // Parse JSON message to extract device_id
        match serde_json::from_str::<serde_json::Value>(message) {
            Ok(json) => {
                // Extract device_id from JSON
                if let Some(device_id) = json.get("device_id").and_then(|v| v.as_str()) {
                    info!("UART message routed to device: {}", device_id);

                    // Use ESP32Manager's TCP message handler (works for UART too)
                    Esp32Manager::handle_tcp_message_bypass(message, device_id, device_store).await;
                } else {
                    warn!("UART message missing device_id field: {}", message);
                }
            }
            Err(e) => {
                warn!("Failed to parse UART message as JSON: {} - Error: {}", message, e);
            }
        }
    }

    /// List available UART ports
    pub fn list_ports() -> Result<Vec<String>, String> {
        match tokio_serial::available_ports() {
            Ok(ports) => {
                let port_names: Vec<String> = ports
                    .into_iter()
                    .map(|p| p.port_name)
                    .collect();
                Ok(port_names)
            }
            Err(e) => Err(format!("Failed to list serial ports: {}", e)),
        }
    }
}

impl Drop for UartConnection {
    fn drop(&mut self) {
        info!("UART connection being dropped");

        // Send shutdown signal if we have one
        if let Some(shutdown_tx) = &self.shutdown_sender {
            let _ = shutdown_tx.send(());
        }
    }
}
