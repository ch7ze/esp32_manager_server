use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, Mutex};
use tokio::time::{timeout, interval, Duration as TokioDuration, MissedTickBehavior};
use tracing::{info, debug};

/// Information about a discovered ESP32 device
#[derive(Debug, Clone)]
pub struct DiscoveredDevice {
    pub port: u16,
    pub data: Vec<u8>,
    pub from_addr: SocketAddr,
    pub message: String,
}

/// UDP Searcher that periodically checks a list of ports for ESP32 devices
pub struct UdpSearcher {
    /// IP address to bind to for listening
    ip_address: IpAddr,
    /// List of ports to check for ESP32 broadcasts
    available_ports: Arc<Mutex<Vec<u16>>>,
    /// Check timer control
    timer_stop_tx: Option<mpsc::UnboundedSender<()>>,
    /// Running state
    is_running: bool,
}

impl UdpSearcher {
    /// Create new UDP searcher
    pub fn new(available_ports: Vec<u16>) -> Self {
        Self {
            ip_address: IpAddr::V4(Ipv4Addr::new(192, 168, 137, 1)),
            available_ports: Arc::new(Mutex::new(available_ports)),
            timer_stop_tx: None,
            is_running: false,
        }
    }
    
    /// Start checking UDP ports with callback
    pub async fn start_checking_udp_ports<F>(
        &mut self,
        port_callback: F,
        interval_opt: Option<u64>,
    ) -> Result<(), String>
    where
        F: Fn(u16) + Send + Sync + 'static,
    {
        if self.is_running {
            return Err("Already running".to_string());
        }
        
        let interval_ms = interval_opt.unwrap_or(1000);
        let (stop_tx, mut stop_rx) = mpsc::unbounded_channel();
        
        self.timer_stop_tx = Some(stop_tx);
        self.is_running = true;
        
        // Start timer task
        let available_ports = Arc::clone(&self.available_ports);
        let ip_address = self.ip_address;
        let callback = Arc::new(port_callback);
        
        tokio::spawn(async move {
            let mut timer = interval(Duration::from_millis(interval_ms));
            timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
            
            loop {
                tokio::select! {
                    _ = timer.tick() => {
                        // Check UDP ports periodically
                        Self::check_udp_ports(Arc::clone(&available_ports), ip_address, Arc::clone(&callback)).await;
                    }
                    _ = stop_rx.recv() => {
                        break;
                    }
                }
            }
        });
        
        info!("UDP searcher started with {}ms interval", interval_ms);
        Ok(())
    }
    
    /// Stop checking UDP ports
    pub async fn stop_checking_udp_ports(&mut self) {
        if let Some(stop_tx) = self.timer_stop_tx.take() {
            let _ = stop_tx.send(());
            self.is_running = false;
            info!("UDP searcher stopped");
        }
    }
    
    /// Add port to search list
    pub async fn add_port(&self, port: u16) {
        let mut ports = self.available_ports.lock().await;
        if !ports.contains(&port) {
            ports.push(port);
        }
    }
    
    /// Remove port from search list
    pub async fn remove_port(&self, port: u16) {
        let mut ports = self.available_ports.lock().await;
        if let Some(pos) = ports.iter().position(|&p| p == port) {
            ports.remove(pos);
        }
    }
    
    /// Replace port list
    pub async fn replace_ports(&self, new_ports: Vec<u16>) {
        let mut ports = self.available_ports.lock().await;
        *ports = new_ports;
    }
    
    // ========================================================================
    // PRIVATE METHODS
    // ========================================================================
    
    /// Check UDP ports for ESP32 broadcasts
    async fn check_udp_ports<F>(
        available_ports: Arc<Mutex<Vec<u16>>>,
        ip_address: IpAddr,
        port_callback: Arc<F>,
    ) where
        F: Fn(u16) + Send + Sync + 'static,
    {
        // Get copy of ports to check
        let ports_to_check = {
            let ports = available_ports.lock().await;
            ports.clone()
        };
        
        for port in ports_to_check {
            match Self::try_receive_from_port(ip_address, port).await {
                Ok(Some(_discovered_device)) => {
                    port_callback(port);
                    
                    {
                        let mut ports = available_ports.lock().await;
                        if let Some(pos) = ports.iter().position(|&p| p == port) {
                            ports.remove(pos);
                        }
                    }
                    
                    return;
                }
                Ok(None) => {}
                Err(_) => {}
            }
        }
    }
    
    /// Try to receive data from a single port
    async fn try_receive_from_port(
        ip_address: IpAddr,
        port: u16,
    ) -> Result<Option<DiscoveredDevice>, String> {
        let bind_addr = SocketAddr::new(ip_address, port);
        
        let socket = match UdpSocket::bind(bind_addr).await {
            Ok(socket) => socket,
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                return Err("Port already in use".to_string());
            }
            Err(e) => {
                return Err(format!("Bind error: {}", e));
            }
        };
        
        let mut buffer = [0u8; 1024];
        match timeout(Duration::from_millis(1000), socket.recv_from(&mut buffer)).await {
            Ok(Ok((bytes_read, from_addr))) => {
                if bytes_read > 0 {
                    let data = buffer[..bytes_read].to_vec();
                    let message = String::from_utf8_lossy(&data).to_string();
                    
                    let device = DiscoveredDevice {
                        port,
                        data,
                        from_addr,
                        message,
                    };
                    
                    Ok(Some(device))
                } else {
                    Ok(None)
                }
            }
            Ok(Err(_)) => Err("Receive error".to_string()),
            Err(_) => Ok(None),
        }
    }
}

impl Drop for UdpSearcher {
    fn drop(&mut self) {
        if let Some(stop_tx) = self.timer_stop_tx.take() {
            let _ = stop_tx.send(());
        }
    }
}

/// Create UDP searcher with default ESP32 port range
pub fn create_esp32_udp_searcher() -> UdpSearcher {
    let ports: Vec<u16> = (60000..=60099).collect();
    UdpSearcher::new(ports)
}