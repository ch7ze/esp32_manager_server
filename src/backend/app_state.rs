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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tokio::sync::RwLock;
    use crate::device_store::create_shared_store;

    async fn create_test_app_state() -> AppState {
        let db = Arc::new(DatabaseManager::new_memory().await.unwrap());
        let device_store = create_shared_store();
        let device_manager = Arc::new(device_manager::DeviceManager::new(device_store.clone()));
        let device_discovery = Arc::new(tokio::sync::Mutex::new(
            device_discovery::DeviceDiscovery::new(device_store.clone()),
        ));
        let mdns = Arc::new(tokio::sync::Mutex::new(
            mdns_server::MdnsServer::new().unwrap(),
        ));
        let uart = Arc::new(tokio::sync::Mutex::new(
            uart_connection::UartConnection::new(
                device_store.clone(),
                Arc::new(RwLock::new(HashMap::new())),
                Arc::new(RwLock::new(HashMap::new())),
                Arc::new(RwLock::new(HashMap::new())),
            ),
        ));

        AppState::new(db, device_store, device_manager, device_discovery, mdns, uart)
    }

    #[tokio::test]
    async fn test_app_state_new() {
        let state = create_test_app_state().await;

        // Alle Felder sollten zugänglich sein
        let _db = &state.db;
        let _store = &state.device_store;
        let _manager = &state.device_manager;
        let _discovery = &state.device_discovery;
        let _mdns = &state.mdns_server;
        let _uart = &state.uart_connection;
    }

    #[tokio::test]
    async fn test_app_state_clone_shares_state() {
        let state = create_test_app_state().await;
        let cloned = state.clone();

        // Arc-Pointer sollten auf dieselben Objekte zeigen
        assert!(Arc::ptr_eq(&state.db, &cloned.db));
        assert!(Arc::ptr_eq(&state.device_store, &cloned.device_store));
        assert!(Arc::ptr_eq(&state.device_manager, &cloned.device_manager));
        assert!(Arc::ptr_eq(&state.device_discovery, &cloned.device_discovery));
        assert!(Arc::ptr_eq(&state.mdns_server, &cloned.mdns_server));
        assert!(Arc::ptr_eq(&state.uart_connection, &cloned.uart_connection));
    }

    #[tokio::test]
    async fn test_app_state_db_accessible() {
        let state = create_test_app_state().await;

        // DB sollte funktionsfähig sein (Guest-User existiert nach init)
        let guest = state.db.get_user_by_id("guest").await.unwrap();
        assert!(guest.is_some());
        assert_eq!(guest.unwrap().email, "guest@system.local");
    }

    #[tokio::test]
    async fn test_app_state_device_store_accessible() {
        let state = create_test_app_state().await;

        // Device Store sollte funktionsfähig sein
        let stats = state.device_store.get_stats().await;
        assert_eq!(stats.total_devices, 0);
        assert_eq!(stats.total_connections, 0);
    }

    #[tokio::test]
    async fn test_app_state_clone_shares_db_mutations() {
        let state = create_test_app_state().await;
        let cloned = state.clone();

        // User über Original erstellen
        let user = crate::database::DatabaseUser::new(
            "test@example.com".to_string(),
            "Test User".to_string(),
            "password123",
        ).unwrap();
        state.db.create_user(user).await.unwrap();

        // Über Clone abrufen — sollte denselben User finden
        let found = cloned.db.get_user_by_email("test@example.com").await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().display_name, "Test User");
    }

    #[tokio::test]
    async fn test_app_state_clone_shares_device_store_mutations() {
        let state = create_test_app_state().await;
        let cloned = state.clone();

        // max_debug_messages über Original ändern
        state.device_store.set_max_debug_messages(500).await;

        // Über Clone prüfen — Stats sollten die Änderung widerspiegeln
        let stats = cloned.device_store.get_stats().await;
        assert_eq!(stats.total_devices, 0);
        // Beide zeigen auf dasselbe Objekt, daher ist die Änderung sichtbar
        assert!(Arc::ptr_eq(&state.device_store, &cloned.device_store));
    }
}
