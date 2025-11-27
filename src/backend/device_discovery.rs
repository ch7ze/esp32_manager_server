// Device Discovery Service - Automatically discovers and manages Device devices

use crate::mdns_discovery::{MdnsDiscovery, create_mdns_discovery, MdnsDiscoveredDevice};
use crate::device_types::{DeviceConfig, DeviceResult};
use crate::device_manager::DeviceManager;
use crate::events::DeviceEvent;
use crate::device_store::DeviceEventStore;
use crate::database::DatabaseManager;

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, debug, warn};

// ============================================================================
// Device DISCOVERY SERVICE - Simplified
// ============================================================================

/// Discovered Device device with discovery metadata
#[derive(Debug, Clone)]
pub struct DiscoveredDevice {
    pub device_config: DeviceConfig,
    pub discovered_at: chrono::DateTime<chrono::Utc>,
    pub udp_port: u16,
    pub mdns_data: Option<MdnsDiscoveredDevice>,
}

/// Device discovery service that integrates with WebSocket system
pub struct DeviceDiscovery {
    mdns_discovery: Option<MdnsDiscovery>,
    discovered_devices: Arc<RwLock<HashMap<String, DiscoveredDevice>>>,
    device_manager: Option<Arc<DeviceManager>>,
    device_store: Arc<DeviceEventStore>,
    db: Option<Arc<DatabaseManager>>,
    is_running: bool,
}

impl DeviceDiscovery {
    /// Create new Device discovery service
    pub fn new(device_store: Arc<DeviceEventStore>) -> Self {
        Self::with_manager(device_store, None, None)
    }

    /// Create new Device discovery service with manager integration
    pub fn with_manager(device_store: Arc<DeviceEventStore>, device_manager: Option<Arc<DeviceManager>>, db: Option<Arc<DatabaseManager>>) -> Self {
        let mdns_discovery = match create_mdns_discovery() {
            Ok(discovery) => Some(discovery),
            Err(e) => {
                tracing::warn!("Failed to create mDNS discovery: {}, falling back to UDP only", e);
                None
            }
        };

        Self {
            mdns_discovery,
            discovered_devices: Arc::new(RwLock::new(HashMap::new())),
            device_manager,
            device_store,
            db,
            is_running: false,
        }
    }
    
    /// Start discovery and broadcast found devices via WebSocket
    pub async fn start_discovery(&mut self) -> DeviceResult<()> {
        if self.is_running {
            return Err(crate::device_types::DeviceError::ConnectionFailed("Already running".to_string()));
        }
        
        self.is_running = true;
        
        let discovered_devices = Arc::clone(&self.discovered_devices);
        let device_store = Arc::clone(&self.device_store);
        let db = self.db.clone();

        // Start mDNS discovery (primary method)
        if let Some(ref mut mdns_discovery) = self.mdns_discovery {
            let discovered_devices_mdns = Arc::clone(&discovered_devices);
            let device_store_mdns = Arc::clone(&device_store);
            let device_manager_clone = self.device_manager.clone();
            let db_clone = db.clone();
            
            mdns_discovery.start_discovery(move |mdns_device: MdnsDiscoveredDevice| {
                tracing::info!("DeviceDiscovery callback triggered for: {}", mdns_device.hostname);
                
                // Use MAC address as device ID instead of hostname
                let device_id = mdns_device.txt_records.get("mac")
                    .map(|mac| mac.replace(':', "-"))  // Konvertiere MAC zu Key-Format mit Bindestrichen
                    .unwrap_or_else(|| format!("device-{}", mdns_device.hostname.replace(".local", "").trim_end_matches('.')));
                let ip = mdns_device.ip_addresses.first().copied()
                    .unwrap_or(IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 100)));
                
                let device_config = DeviceConfig::new(
                    device_id.clone(),
                    ip,
                    3232, // Device TCP port (same as UDP port)
                    3232, // Device UDP port
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
                        let discovered_device = DiscoveredDevice {
                            device_config: device_config_clone.clone(),
                            discovered_at,
                            udp_port: mdns_device.port,
                            mdns_data: Some(mdns_device.clone()),
                        };
                        devices.insert(device_id_clone.clone(), discovered_device);
                        tracing::info!("Device device stored in HashMap: {}", device_id_clone);
                    } else {
                        tracing::warn!("Could not acquire write lock for discovered devices");
                    }
                }
                
                // Use thread::spawn for async operations since we're not in tokio context
                let device_store_spawn = Arc::clone(&device_store_clone);
                let device_id_spawn = device_id_clone.clone();
                let device_config_spawn = device_config_clone.clone();
                let device_manager_spawn = device_manager_clone.clone();
                let db_spawn = db_clone.clone();

                std::thread::spawn(move || {
                    tracing::info!("DeviceDiscovery thread spawned for: {}", device_id_spawn);
                    
                    // Create a new tokio runtime for this thread
                    let rt = match tokio::runtime::Runtime::new() {
                        Ok(rt) => rt,
                        Err(e) => {
                            tracing::error!("Failed to create tokio runtime: {}", e);
                            return;
                        }
                    };
                    
                    rt.block_on(async move {
                        // Extract MAC address from mDNS data (original format with colons for display)
                        let mac_address = mdns_device.txt_records.get("mac").cloned();

                        // Extract mDNS hostname without .local suffix
                        let mdns_hostname_string = mdns_device.hostname.replace(".local", "").trim_end_matches('.').to_string();
                        let mdns_hostname = Some(mdns_hostname_string.clone());

                        // Create UDP device config with MAC address as device_id
                        let (final_device_id, udp_device_config) = if let Some(ref mac) = mac_address {
                            let config = crate::device_types::DeviceConfig::new_udp(
                                mac.clone(), // MAC address IS the device_id
                                device_config_spawn.ip_address,
                                device_config_spawn.udp_port,
                            );
                            (mac.clone(), config)
                        } else {
                            // No MAC address - use original device_id
                            (device_id_spawn.clone(), device_config_spawn.clone())
                        };

                        // Broadcast discovery event to all WebSocket clients
                        let discovery_event = DeviceEvent::device_discovered(
                            final_device_id.clone(),
                            device_config_spawn.ip_address.to_string(),
                            device_config_spawn.tcp_port,
                            device_config_spawn.udp_port,
                            discovered_at.to_rfc3339(),
                            mac_address.clone(),
                            mdns_hostname,
                        );

                        match device_store_spawn.broadcast_event("system", discovery_event, "system").await {
                            Ok(_) => tracing::info!("Device discovery WebSocket event sent for: {}", final_device_id),
                            Err(e) => tracing::warn!("Failed to broadcast Device discovery event: {}", e),
                        }

                        tracing::info!("Device device discovered via mDNS: {} (original: {}, MAC: {:?}) at {}",
                            final_device_id, device_id_spawn, mac_address, ip);

                        // Save device to database
                        if let Some(db) = &db_spawn {
                            let device_name = if !mdns_hostname_string.is_empty() {
                                mdns_hostname_string.clone()
                            } else {
                                format!("Device-{}", &final_device_id[..8.min(final_device_id.len())])
                            };

                            if let Err(e) = db.upsert_discovered_device(
                                final_device_id.clone(),
                                device_name,
                                Some(device_config_spawn.ip_address.to_string()),
                                Some("tcp".to_string()),  // Connection type: TCP
                            ).await {
                                tracing::warn!("Failed to save discovered device to database: {}", e);
                            } else {
                                tracing::info!("Saved discovered device to database: {}", final_device_id);
                            }
                        }

                        // Automatically add device to manager if available (but don't connect yet)
                        if let Some(manager) = &device_manager_spawn {
                            tracing::info!("Adding discovered Device to manager: {} (MAC as device_id)", final_device_id);

                            // Add UDP device with MAC as device_id
                            if let Err(e) = manager.add_device(udp_device_config).await {
                                tracing::warn!("Failed to add discovered device to manager: {}", e);
                            } else {
                                tracing::info!("Successfully added Device {} to manager (not connected yet)", final_device_id);
                            }
                        }
                    });
                });
            }).await.map_err(|e| crate::device_types::DeviceError::ConnectionFailed(e))?;
            
            info!("mDNS discovery started successfully");
        } else {
            warn!("mDNS discovery not available, using UDP fallback only");
        }
        
        
        info!("Device discovery service started");
        Ok(())
    }
    
    /// Stop discovery
    pub async fn stop_discovery(&mut self) {
        if self.is_running {
            // Stop mDNS discovery
            if let Some(ref mut mdns_discovery) = self.mdns_discovery {
                mdns_discovery.stop_discovery().await;
            }
            
            
            self.is_running = false;
            info!("Device discovery service stopped");
        }
    }
    
    /// Get all discovered devices
    pub async fn get_discovered_devices(&self) -> HashMap<String, DiscoveredDevice> {
        self.discovered_devices.read().await.clone()
    }
    
}

// Note: Default implementation is not available since DeviceEventStore is required

impl Drop for DeviceDiscovery {
    fn drop(&mut self) {
        if self.is_running {
            // Cleanup handled by mDNS discovery
            debug!("DeviceDiscovery dropped while running");
        }
    }
}
