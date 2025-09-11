// ESP32 Discovery Service - Automatically discovers and manages ESP32 devices

use crate::udp_searcher::{UdpSearcher, create_esp32_udp_searcher};
use crate::mdns_discovery::{MdnsDiscovery, create_mdns_discovery, MdnsEsp32Device};
use crate::esp32_types::{Esp32DeviceConfig, Esp32Result};
use crate::events::DeviceEvent;
use crate::device_store::DeviceEventStore;

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, debug, warn};

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
    mdns_discovery: Option<MdnsDiscovery>,
    discovered_devices: Arc<RwLock<HashMap<String, Esp32DeviceConfig>>>,
    device_store: Arc<DeviceEventStore>,
    is_running: bool,
}

impl Esp32Discovery {
    /// Create new ESP32 discovery service
    pub fn new(device_store: Arc<DeviceEventStore>) -> Self {
        let mdns_discovery = match create_mdns_discovery() {
            Ok(discovery) => Some(discovery),
            Err(e) => {
                tracing::warn!("Failed to create mDNS discovery: {}, falling back to UDP only", e);
                None
            }
        };
        
        Self {
            udp_searcher: create_esp32_udp_searcher(),
            mdns_discovery,
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
        
        // Start mDNS discovery (primary method)
        if let Some(ref mut mdns_discovery) = self.mdns_discovery {
            let discovered_devices_mdns = Arc::clone(&discovered_devices);
            let device_store_mdns = Arc::clone(&device_store);
            
            mdns_discovery.start_discovery(move |mdns_device: MdnsEsp32Device| {
                tracing::info!("ESP32Discovery callback triggered for: {}", mdns_device.hostname);
                
                let device_id = format!("esp32-{}", mdns_device.hostname.replace(".local", ""));
                let ip = mdns_device.ip_addresses.first().copied()
                    .unwrap_or(IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 100)));
                
                let device_config = Esp32DeviceConfig::new(
                    device_id.clone(),
                    ip,
                    23, // Default TCP port
                    mdns_device.port,
                );
                
                let discovered_at = chrono::Utc::now();
                
                // Store and broadcast discovered device
                let discovered_devices = Arc::clone(&discovered_devices_mdns);
                let device_store = Arc::clone(&device_store_mdns);
                
                // Simplified: try to avoid tokio::spawn issues
                let discovered_devices_clone = Arc::clone(&discovered_devices);
                let device_store_clone = Arc::clone(&device_store);
                let device_id_clone = device_id.clone();
                let device_config_clone = device_config.clone();
                
                // Store device synchronously first
                {
                    if let Ok(mut devices) = discovered_devices_clone.try_write() {
                        devices.insert(device_id_clone.clone(), device_config_clone.clone());
                        tracing::info!("ESP32 device stored in HashMap: {}", device_id_clone);
                    } else {
                        tracing::warn!("Could not acquire write lock for discovered devices");
                    }
                }
                
                // Use thread::spawn for async operations since we're not in tokio context
                let device_store_spawn = Arc::clone(&device_store_clone);
                let device_id_spawn = device_id_clone.clone();
                let device_config_spawn = device_config_clone.clone();
                
                std::thread::spawn(move || {
                    tracing::info!("ESP32Discovery thread spawned for: {}", device_id_spawn);
                    
                    // Create a new tokio runtime for this thread
                    let rt = match tokio::runtime::Runtime::new() {
                        Ok(rt) => rt,
                        Err(e) => {
                            tracing::error!("Failed to create tokio runtime: {}", e);
                            return;
                        }
                    };
                    
                    rt.block_on(async move {
                        // Broadcast discovery event to all WebSocket clients
                        let discovery_event = DeviceEvent::esp32_device_discovered(
                            device_id_spawn.clone(),
                            device_config_spawn.ip_address.to_string(),
                            device_config_spawn.tcp_port,
                            device_config_spawn.udp_port,
                            discovered_at.to_rfc3339(),
                        );
                        
                        match device_store_spawn.broadcast_event("system", discovery_event, "system").await {
                            Ok(_) => tracing::info!("ESP32 discovery WebSocket event sent for: {}", device_id_spawn),
                            Err(e) => tracing::warn!("Failed to broadcast ESP32 discovery event: {}", e),
                        }
                        
                        tracing::info!("ESP32 device discovered via mDNS: {} at {}", device_id_spawn, ip);
                    });
                });
            }).await.map_err(|e| crate::esp32_types::Esp32Error::ConnectionFailed(e))?;
            
            info!("mDNS discovery started successfully");
        } else {
            warn!("mDNS discovery not available, using UDP fallback only");
        }
        
        // Keep UDP searcher as fallback (but don't start it automatically)
        // Uncomment the following lines if you want UDP as backup:
        /*
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
        */
        // UDP searcher is kept as backup but not started automatically
        
        info!("ESP32 discovery service started");
        Ok(())
    }
    
    /// Stop discovery
    pub async fn stop_discovery(&mut self) {
        if self.is_running {
            // Stop mDNS discovery
            if let Some(ref mut mdns_discovery) = self.mdns_discovery {
                mdns_discovery.stop_discovery().await;
            }
            
            // Stop UDP searcher (if it was running)
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