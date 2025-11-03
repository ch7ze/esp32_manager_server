// ============================================================================
// APP_STATE.RS - CENTRALIZED APPLICATION STATE
// Single source of truth for application state definition
// ============================================================================

use std::sync::Arc;
use crate::database::DatabaseManager;
use crate::device_store::SharedDeviceStore;
use crate::esp32_manager;
use crate::esp32_discovery;
use crate::mdns_server;
use crate::uart_connection;

/// Central application state shared across all handlers and services
///
/// This struct contains all the core dependencies needed by the application.
/// It is cloneable (cheap Arc clones) and can be passed to all Axum handlers
/// via the State extractor.
///
/// # Fields
///
/// * `db` - Database manager for user and device persistence (SQLite)
/// * `device_store` - In-memory store for device events and WebSocket connections
/// * `esp32_manager` - ESP32 device manager for WiFi-connected devices
/// * `esp32_discovery` - ESP32 discovery service for finding devices on the network
/// * `mdns_server` - mDNS server for service discovery (esp-server.local)
/// * `uart_connection` - UART connection manager for serial-connected ESP32 devices
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<DatabaseManager>,
    pub device_store: SharedDeviceStore,
    pub esp32_manager: Arc<esp32_manager::Esp32Manager>,
    pub esp32_discovery: Arc<tokio::sync::Mutex<esp32_discovery::Esp32Discovery>>,
    pub mdns_server: Arc<tokio::sync::Mutex<mdns_server::MdnsServer>>,
    pub uart_connection: Arc<tokio::sync::Mutex<uart_connection::UartConnection>>,
}

impl AppState {
    /// Create a new AppState instance with all dependencies
    ///
    /// # Arguments
    ///
    /// * `db` - Database manager instance
    /// * `device_store` - Device event store instance
    /// * `esp32_manager` - ESP32 manager instance
    /// * `esp32_discovery` - ESP32 discovery service instance
    /// * `mdns_server` - mDNS server instance
    /// * `uart_connection` - UART connection manager instance
    #[allow(dead_code)]
    pub fn new(
        db: Arc<DatabaseManager>,
        device_store: SharedDeviceStore,
        esp32_manager: Arc<esp32_manager::Esp32Manager>,
        esp32_discovery: Arc<tokio::sync::Mutex<esp32_discovery::Esp32Discovery>>,
        mdns_server: Arc<tokio::sync::Mutex<mdns_server::MdnsServer>>,
        uart_connection: Arc<tokio::sync::Mutex<uart_connection::UartConnection>>,
    ) -> Self {
        Self {
            db,
            device_store,
            esp32_manager,
            esp32_discovery,
            mdns_server,
            uart_connection,
        }
    }
}
