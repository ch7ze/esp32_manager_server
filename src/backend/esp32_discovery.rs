// ESP32 Discovery Service - Automatically discovers and manages ESP32 devices

use crate::udp_searcher::{UdpSearcher, create_esp32_udp_searcher};
use crate::esp32_types::{Esp32DeviceConfig, Esp32Result};
use crate::events::DeviceEvent;
use crate::device_store::DeviceEventStore;

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{info, debug};

// ============================================================================
// ESP32 DISCOVERY SERVICE - Simplified
// ============================================================================

/// Discovered ESP32 device with discovery metadata
#[derive(Debug, Clone)]
pub struct DiscoveredEsp32Device {
    pub device_config: Esp32DeviceConfig,
    pub discovered_at: chrono::DateTime<chrono::Utc>,
    pub udp_port: u16,
}

/// ESP32 discovery service that integrates with WebSocket system
pub struct Esp32Discovery {
    udp_searcher: UdpSearcher,
    discovered_devices: Arc<RwLock<HashMap<String, Esp32DeviceConfig>>>,
    device_store: Arc<DeviceEventStore>,
    is_running: bool,
}

impl Esp32Discovery {
    /// Create new ESP32 discovery service
    pub fn new(device_store: Arc<DeviceEventStore>) -> Self {
        Self {
            udp_searcher: create_esp32_udp_searcher(),
            discovered_devices: Arc::new(RwLock::new(HashMap::new())),
            device_store,
            is_running: false,
        }
    }
    
    /// Start discovery and broadcast found devices via WebSocket
    pub async fn start_discovery(&mut self) -> Esp32Result<()> {
        if self.is_running {
            return Err(crate::esp32_types::Esp32Error::ConnectionFailed("Already running".to_string()));
        }
        
        self.is_running = true;
        
        let discovered_devices = Arc::clone(&self.discovered_devices);
        let device_store = Arc::clone(&self.device_store);
        
        // Start UDP searcher with port callback
        self.udp_searcher.start_checking_udp_ports(move |port| {
            let device_id = format!("esp32-port-{}", port);
            let ip = IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 100));
            
            let device_config = Esp32DeviceConfig::new(
                device_id.clone(),
                ip,
                23,
                port,
            );
            
            let discovered_at = chrono::Utc::now();
            
            // Store discovered device
            let discovered_devices = Arc::clone(&discovered_devices);
            let device_store = Arc::clone(&device_store);
            tokio::spawn(async move {
                {
                    let mut devices = discovered_devices.write().await;
                    devices.insert(device_id.clone(), device_config.clone());
                }
                
                // Broadcast discovery event to all WebSocket clients
                let discovery_event = DeviceEvent::esp32_device_discovered(
                    device_id.clone(),
                    device_config.ip_address.to_string(),
                    device_config.tcp_port,
                    device_config.udp_port,
                    discovered_at.to_rfc3339(),
                );
                
                device_store.broadcast_event("system", discovery_event, "system").await.unwrap_or_else(|e| {
                    tracing::warn!("Failed to broadcast ESP32 discovery event: {}", e);
                });
                info!("ESP32 device discovered and broadcasted: {}", device_id);
            });
        }, None).await.map_err(|e| crate::esp32_types::Esp32Error::ConnectionFailed(e))?;
        
        info!("ESP32 discovery service started");
        Ok(())
    }
    
    /// Stop discovery
    pub async fn stop_discovery(&mut self) {
        if self.is_running {
            self.udp_searcher.stop_checking_udp_ports().await;
            self.is_running = false;
            info!("ESP32 discovery service stopped");
        }
    }
    
    /// Get all discovered devices
    pub async fn get_discovered_devices(&self) -> HashMap<String, Esp32DeviceConfig> {
        self.discovered_devices.read().await.clone()
    }
    
    /// Add port to scan
    pub async fn add_scan_port(&self, port: u16) {
        self.udp_searcher.add_port(port).await;
    }
    
    /// Remove port from scanning
    pub async fn remove_scan_port(&self, port: u16) {
        self.udp_searcher.remove_port(port).await;
    }
}

// Note: Default implementation is not available since DeviceEventStore is required

impl Drop for Esp32Discovery {
    fn drop(&mut self) {
        if self.is_running {
            // Cannot call async method in Drop, but the UdpSearcher will clean up itself
            debug!("ESP32Discovery dropped while running");
        }
    }
}