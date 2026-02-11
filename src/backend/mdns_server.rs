use mdns_sd::{ServiceDaemon, ServiceInfo};
use std::collections::HashMap;
use std::net::IpAddr;
use tracing::{info, warn};
use tokio::sync::mpsc;

/// mDNS server for advertising the Device Manager Server
pub struct MdnsServer {
    daemon: Option<ServiceDaemon>,
    service_infos: Vec<ServiceInfo>,
    stop_tx: Option<mpsc::UnboundedSender<()>>,
    is_running: bool,
}

impl MdnsServer {
    /// Create new mDNS server
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            daemon: None,
            service_infos: Vec::new(),
            stop_tx: None,
            is_running: false,
        })
    }

    /// Start advertising the server via mDNS
    pub async fn start_advertising(&mut self, port: u16) -> Result<(), String> {
        if self.is_running {
            return Err("mDNS server already running".to_string());
        }

        // Create mDNS daemon
        let daemon = ServiceDaemon::new()
            .map_err(|e| format!("Failed to create mDNS daemon: {}", e))?;

        // Get local IP addresses (IPv4 and IPv6)
        let local_ips = self.get_local_ip_addresses()?;
        if local_ips.is_empty() {
            return Err("No local IP addresses found".to_string());
        }

        // Register ALL collected IPs, not just the first IPv4.
        // mdns-sd filters A-Record responses via get_addrs_on_intf(): only
        // addresses in the same subnet as the incoming query's interface are
        // returned. A single registered IP means queries arriving on any other
        // interface (different subnet) get no answer. ESP32 mdns_query_a()
        // relies strictly on an A-Record reply, so it fails silently.
        // ServiceInfo::new accepts multiple IPs as a comma-separated string.
        let ip_list: String = local_ips.iter()
            .map(|ip| ip.to_string())
            .collect::<Vec<_>>()
            .join(",");

        info!("Registering mDNS with all IPs: {}", ip_list);

        // Create TXT records with server information
        let mut properties = HashMap::new();
        properties.insert("version".to_string(), "1.0".to_string());
        properties.insert("path".to_string(), "/".to_string());
        properties.insert("type".to_string(), "device-manager".to_string());
        properties.insert("protocol".to_string(), "http".to_string());

        // Register service with all local IP addresses
        let service_info = ServiceInfo::new(
            "_http._tcp.local.",
            "device-manager",
            "device-manager.local.",
            ip_list.as_str(),
            port,
            properties,
        ).map_err(|e| format!("Failed to create service info: {}", e))?;

        // Register the service
        daemon.register(service_info.clone())
            .map_err(|e| format!("Failed to register mDNS service: {}", e))?;


        self.daemon = Some(daemon);
        self.service_infos = vec![service_info];
        self.is_running = true;

        // Start keep-alive task
        let (stop_tx, mut stop_rx) = mpsc::unbounded_channel();
        self.stop_tx = Some(stop_tx);

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = stop_rx.recv() => {
                        info!("Stopping mDNS server advertising");
                        break;
                    }
                    _ = tokio::time::sleep(tokio::time::Duration::from_secs(30)) => {
                        // Keep daemon alive by doing nothing - the service stays registered
                    }
                }
            }
        });

        Ok(())
    }

    /// Stop mDNS advertising
    pub async fn stop_advertising(&mut self) {
        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(());
        }

        if let Some(daemon) = self.daemon.take() {
            // Unregister all services
            for service_info in self.service_infos.drain(..) {
                if let Err(e) = daemon.unregister(service_info.get_fullname()) {
                    warn!("Failed to unregister mDNS service: {}", e);
                }
            }

            if let Err(e) = daemon.shutdown() {
                warn!("Failed to shutdown mDNS daemon: {}", e);
            }
        }

        self.is_running = false;
        info!("mDNS server advertising stopped");
    }

    /// Get ALL local IP addresses on all interfaces (for mDNS to respond on all networks)
    fn get_local_ip_addresses(&self) -> Result<Vec<IpAddr>, String> {
        use if_addrs::get_if_addrs;
        use std::io::Write;

        // Open debug log file
        let mut debug_log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("mdns_debug.log")
            .ok();

        let mut addresses = Vec::new();

        // Get all network interfaces
        match get_if_addrs() {
            Ok(interfaces) => {
                for iface in interfaces {
                    let ip = iface.ip();

                    // Skip loopback addresses (127.0.0.1, ::1)
                    if ip.is_loopback() {
                        let msg = format!("Skipping loopback address: {} on {}\n", ip, iface.name);
                        info!("{}", msg.trim());
                        if let Some(ref mut log) = debug_log {
                            let _ = log.write_all(msg.as_bytes());
                        }
                        continue;
                    }

                    // Skip VPN interfaces (NordLynx, OpenVPN, WireGuard, etc.)
                    let iface_name_lower = iface.name.to_lowercase();
                    if iface_name_lower.contains("nordlynx")
                        || iface_name_lower.contains("vpn")
                        || iface_name_lower.contains("tun")
                        || iface_name_lower.contains("tap")
                        || iface_name_lower.contains("wireguard") {
                        let msg = format!("Skipping VPN interface: {} ({})\n", iface.name, ip);
                        info!("{}", msg.trim());
                        if let Some(ref mut log) = debug_log {
                            let _ = log.write_all(msg.as_bytes());
                        }
                        continue;
                    }

                    // Skip virtual/VM interfaces (VirtualBox, VMware, Docker)
                    if iface_name_lower.contains("virtualbox")
                        || iface_name_lower.contains("vmware")
                        || iface_name_lower.contains("vethernet")
                        || iface_name_lower.contains("docker") {
                        let msg = format!("Skipping virtual interface: {} ({})\n", iface.name, ip);
                        info!("{}", msg.trim());
                        if let Some(ref mut log) = debug_log {
                            let _ = log.write_all(msg.as_bytes());
                        }
                        continue;
                    }

                    // Skip VirtualBox Host-Only networks (192.168.56.x, 192.168.99.x)
                    if let IpAddr::V4(ipv4) = ip {
                        let octets = ipv4.octets();
                        if (octets[0] == 192 && octets[1] == 168 && (octets[2] == 56 || octets[2] == 99))
                            || (octets[0] == 10 && octets[1] == 5) {  // NordLynx uses 10.5.x.x
                            let msg = format!("Skipping VM/VPN network: {} ({})\n", iface.name, ip);
                            info!("{}", msg.trim());
                            if let Some(ref mut log) = debug_log {
                                let _ = log.write_all(msg.as_bytes());
                            }
                            continue;
                        }
                    }

                    // Skip link-local IPv6 addresses (fe80::) - these cause mDNS errors
                    if let IpAddr::V6(ipv6) = ip {
                        if ipv6.segments()[0] == 0xfe80 {
                            let msg = format!("Skipping link-local IPv6 address: {} on {}\n", ip, iface.name);
                            info!("{}", msg.trim());
                            if let Some(ref mut log) = debug_log {
                                let _ = log.write_all(msg.as_bytes());
                            }
                            continue;
                        }
                    }

                    let msg = format!("Registering mDNS on interface: {} with IP {} ({})\n",
                          iface.name,
                          ip,
                          if ip.is_ipv4() { "IPv4" } else { "IPv6" });
                    info!("{}", msg.trim());
                    if let Some(ref mut log) = debug_log {
                        let _ = log.write_all(msg.as_bytes());
                    }
                    addresses.push(ip);
                }
            }
            Err(e) => {
                return Err(format!("Failed to enumerate network interfaces: {}", e));
            }
        }

        if addresses.is_empty() {
            return Err("No usable IP addresses found on any interface".to_string());
        }

        let msg = format!("Total mDNS IP addresses registered: {}\n", addresses.len());
        info!("{}", msg.trim());
        if let Some(ref mut log) = debug_log {
            let _ = log.write_all(msg.as_bytes());
            let _ = log.flush();
        }

        Ok(addresses)
    }

    /// Check if server is running
    pub fn is_running(&self) -> bool {
        self.is_running
    }
}

impl Drop for MdnsServer {
    fn drop(&mut self) {
        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(());
        }

        if let Some(daemon) = self.daemon.take() {
            // Unregister all services
            for service_info in self.service_infos.drain(..) {
                let _ = daemon.unregister(service_info.get_fullname());
            }
            let _ = daemon.shutdown();
        }
    }
}