// ============================================================================
// APP_STATE.RS - CENTRALIZED APPLICATION STATE
// Single source of truth for application state definition
// ============================================================================

use std::sync::Arc;
use crate::database::DatabaseManager;
use crate::device_store::SharedDeviceStore;
use crate::device_manager;
use crate::device_discovery;
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
/// * `device_manager` - Device manager for WiFi-connected devices
/// * `device_discovery` - Device discovery service for finding devices on the network
/// * `mdns_server` - mDNS server for service discovery (esp-server.local)
/// * `uart_connection` - UART connection manager for serial-connected devices
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<DatabaseManager>,
    pub device_store: SharedDeviceStore,
    pub device_manager: Arc<device_manager::DeviceManager>,
    pub device_discovery: Arc<tokio::sync::Mutex<device_discovery::DeviceDiscovery>>,
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
    /// * `device_manager` - Device manager instance
    /// * `device_discovery` - Device discovery service instance
    /// * `mdns_server` - mDNS server instance
    /// * `uart_connection` - UART connection manager instance
    #[allow(dead_code)]
    pub fn new(
        db: Arc<DatabaseManager>,
        device_store: SharedDeviceStore,
        device_manager: Arc<device_manager::DeviceManager>,
        device_discovery: Arc<tokio::sync::Mutex<device_discovery::DeviceDiscovery>>,
        mdns_server: Arc<tokio::sync::Mutex<mdns_server::MdnsServer>>,
        uart_connection: Arc<tokio::sync::Mutex<uart_connection::UartConnection>>,
    ) -> Self {
        Self {
            db,
            device_store,
            device_manager,
            device_discovery,
            mdns_server,
            uart_connection,
        }
    }
}
