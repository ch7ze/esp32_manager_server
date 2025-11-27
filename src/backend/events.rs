// ============================================================================
// DEVICE EVENTS - Event Definitions for Client-Server Communication
// ============================================================================

use serde::{Deserialize, Serialize};

// ============================================================================
// CLIENT-SERVER COMMUNICATION MESSAGES
// ============================================================================

/// Subscription type for device events
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SubscriptionType {
    /// Light subscription: only connection status events
    Light,
    /// Full subscription: all events (variables, UDP, connection status, etc.)
    Full,
}

impl Default for SubscriptionType {
    fn default() -> Self {
        SubscriptionType::Full
    }
}

/// WebSocket messages sent from Client to Server
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    #[serde(rename = "registerForDevice")]
    RegisterForDevice {
        #[serde(rename = "deviceId")]
        device_id: String,
        #[serde(rename = "subscriptionType", default)]
        subscription_type: SubscriptionType,
    },
    #[serde(rename = "unregisterForDevice")]
    UnregisterForDevice {
        #[serde(rename = "deviceId")]
        device_id: String
    },
    #[serde(rename = "deviceEvent")]
    DeviceEvent {
        #[serde(rename = "deviceId")]
        device_id: String,
        #[serde(rename = "eventsForDevice")]
        events_for_device: Vec<DeviceEvent>
    },
}

/// WebSocket messages sent from Server to Client
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ServerMessage {
    /// Device events message
    DeviceEvents {
        #[serde(rename = "deviceId")]
        device_id: String,
        #[serde(rename = "eventsForDevice")]
        events_for_device: Vec<DeviceEvent>,
    },
    /// Heartbeat pong response
    Pong {
        #[serde(rename = "type")]
        message_type: String,
        timestamp: Option<u64>,
    },
}

impl ServerMessage {
    /// Create a pong response message
    pub fn pong(timestamp: Option<u64>) -> Self {
        ServerMessage::Pong {
            message_type: "pong".to_string(),
            timestamp,
        }
    }
    
    /// Create a device events message
    pub fn device_events(device_id: String, events_for_device: Vec<DeviceEvent>) -> Self {
        ServerMessage::DeviceEvents {
            device_id,
            events_for_device,
        }
    }
}

// ============================================================================
// DEVICE EVENT DEFINITIONS - Compatible with Frontend EventBus
// ============================================================================

/// Device events for device management and control
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event")]
pub enum DeviceEvent {
    #[serde(rename = "deviceCommand")]
    DeviceCommand {
        #[serde(rename = "deviceId")]
        device_id: String,
        command: serde_json::Value,
    },
    #[serde(rename = "deviceStatusUpdate")]
    DeviceStatusUpdate {
        status: String,
        #[serde(rename = "ipAddress")]
        ip_address: Option<String>,
        #[serde(rename = "firmwareVersion")]
        firmware_version: Option<String>,
    },
    #[serde(rename = "deviceConfigUpdate")]
    DeviceConfigUpdate {
        config: serde_json::Value,
    },
    #[serde(rename = "deviceSensorData")]
    DeviceSensorData {
        sensor: String,
        value: serde_json::Value,
        timestamp: i64,
    },
    #[serde(rename = "userJoined")]
    UserJoined {
        #[serde(rename = "userId")]
        user_id: String,
        #[serde(rename = "displayName")]
        display_name: String,
        #[serde(rename = "userColor")]
        user_color: String,
    },
    #[serde(rename = "userLeft")]
    UserLeft {
        #[serde(rename = "userId")]
        user_id: String,
        #[serde(rename = "displayName")]
        display_name: String,
        #[serde(rename = "userColor")]
        user_color: String,
    },
    // Device-specific events
    #[serde(rename = "DeviceVariableUpdate")]
    DeviceVariableUpdate {
        #[serde(rename = "deviceId")]
        device_id: String,
        #[serde(rename = "variableName")]
        variable_name: String,
        #[serde(rename = "variableValue")]
        variable_value: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        min: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<u64>,
    },
    #[serde(rename = "DeviceStartOptions")]
    DeviceStartOptions {
        #[serde(rename = "deviceId")]
        device_id: String,
        options: Vec<String>,
    },
    #[serde(rename = "DeviceChangeableVariables")]
    DeviceChangeableVariables {
        #[serde(rename = "deviceId")]
        device_id: String,
        variables: Vec<serde_json::Value>,
    },
    #[serde(rename = "DeviceUdpBroadcast")]
    DeviceUdpBroadcast {
        #[serde(rename = "deviceId")]
        device_id: String,
        message: String,
        #[serde(rename = "fromIp")]
        from_ip: String,
        #[serde(rename = "fromPort")]
        from_port: u16,
    },
    #[serde(rename = "DeviceConnectionStatus")]
    DeviceConnectionStatus {
        #[serde(rename = "deviceId")]
        device_id: String,
        connected: bool,
        #[serde(rename = "deviceIp")]
        device_ip: String,
        #[serde(rename = "tcpPort")]
        tcp_port: u16,
        #[serde(rename = "udpPort")]
        udp_port: u16,
    },
    #[serde(rename = "DeviceDeviceInfo")]
    DeviceDeviceInfo {
        #[serde(rename = "deviceId")]
        device_id: String,
        #[serde(rename = "deviceName")]
        device_name: Option<String>,
        #[serde(rename = "firmwareVersion")]
        firmware_version: Option<String>,
        uptime: Option<u64>,
    },
    #[serde(rename = "DeviceDiscovered")]
    DeviceDiscovered {
        #[serde(rename = "deviceId")]
        device_id: String,
        #[serde(rename = "deviceIp")]
        device_ip: String,
        #[serde(rename = "tcpPort")]
        tcp_port: u16,
        #[serde(rename = "udpPort")]
        udp_port: u16,
        #[serde(rename = "discoveredAt")]
        discovered_at: String,
        #[serde(rename = "macAddress")]
        mac_address: Option<String>,
        #[serde(rename = "mdnsHostname")]
        mdns_hostname: Option<String>,
    },
}


// ============================================================================
// EVENT METADATA - For Event Sourcing & Synchronization
// ============================================================================

/// Complete event with metadata for event sourcing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventWithMetadata {
    pub event: DeviceEvent,
    pub id: String,
    pub timestamp: i64,
    pub user_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_replay: Option<bool>,
}

// ============================================================================
// CONVENIENCE CONSTRUCTORS
// ============================================================================

impl DeviceEvent {
    pub fn device_command(device_id: String, command: serde_json::Value) -> Self {
        DeviceEvent::DeviceCommand { device_id, command }
    }
    
    pub fn device_status_update(status: String, ip_address: Option<String>, firmware_version: Option<String>) -> Self {
        DeviceEvent::DeviceStatusUpdate { status, ip_address, firmware_version }
    }
    
    pub fn device_config_update(config: serde_json::Value) -> Self {
        DeviceEvent::DeviceConfigUpdate { config }
    }
    
    pub fn device_sensor_data(sensor: String, value: serde_json::Value, timestamp: i64) -> Self {
        DeviceEvent::DeviceSensorData { sensor, value, timestamp }
    }
    
    pub fn user_joined(user_id: String, display_name: String, user_color: String) -> Self {
        DeviceEvent::UserJoined { user_id, display_name, user_color }
    }
    
    pub fn user_left(user_id: String, display_name: String, user_color: String) -> Self {
        DeviceEvent::UserLeft { user_id, display_name, user_color }
    }
    
    // Device-specific event constructors with device_id
    pub fn device_command_for_device(device_id: String, command: serde_json::Value) -> Self {
        DeviceEvent::DeviceCommand {
            device_id,
            command
        }
    }
    
    pub fn device_variable_update(device_id: String, variable_name: String, variable_value: String) -> Self {
        DeviceEvent::DeviceVariableUpdate { device_id, variable_name, variable_value, min: None, max: None }
    }

    pub fn device_variable_update_with_range(
        device_id: String,
        variable_name: String,
        variable_value: String,
        min: Option<u64>,
        max: Option<u64>
    ) -> Self {
        DeviceEvent::DeviceVariableUpdate { device_id, variable_name, variable_value, min, max }
    }
    
    pub fn device_start_options(device_id: String, options: Vec<String>) -> Self {
        DeviceEvent::DeviceStartOptions { device_id, options }
    }
    
    pub fn device_changeable_variables(device_id: String, variables: Vec<serde_json::Value>) -> Self {
        DeviceEvent::DeviceChangeableVariables { device_id, variables }
    }
    
    pub fn device_udp_broadcast(device_id: String, message: String, from_ip: String, from_port: u16) -> Self {
        DeviceEvent::DeviceUdpBroadcast { device_id, message, from_ip, from_port }
    }
    
    pub fn device_connection_status(device_id: String, connected: bool, device_ip: String, tcp_port: u16, udp_port: u16) -> Self {
        DeviceEvent::DeviceConnectionStatus { device_id, connected, device_ip, tcp_port, udp_port }
    }
    
    pub fn device_device_info(device_id: String, device_name: Option<String>, firmware_version: Option<String>, uptime: Option<u64>) -> Self {
        DeviceEvent::DeviceDeviceInfo { device_id, device_name, firmware_version, uptime }
    }
    
    pub fn device_discovered(device_id: String, device_ip: String, tcp_port: u16, udp_port: u16, discovered_at: String, mac_address: Option<String>, mdns_hostname: Option<String>) -> Self {
        DeviceEvent::DeviceDiscovered { device_id, device_ip, tcp_port, udp_port, discovered_at, mac_address, mdns_hostname }
    }
}

// ============================================================================
// VALIDATION HELPERS
// ============================================================================

impl DeviceEvent {
    /// Validate that the event has all required data for its type
    pub fn validate(&self) -> Result<(), String> {
        match self {
            DeviceEvent::DeviceCommand { device_id, .. } => {
                if device_id.is_empty() {
                    Err("DeviceCommand requires non-empty device_id".to_string())
                } else {
                    Ok(())
                }
            },
            DeviceEvent::DeviceStatusUpdate { status, .. } => {
                if status.is_empty() {
                    Err("DeviceStatusUpdate requires non-empty status".to_string())
                } else {
                    Ok(())
                }
            },
            DeviceEvent::DeviceConfigUpdate { .. } => Ok(()), // Config can be any JSON
            DeviceEvent::DeviceSensorData { sensor, .. } => {
                if sensor.is_empty() {
                    Err("DeviceSensorData requires non-empty sensor name".to_string())
                } else {
                    Ok(())
                }
            },
            DeviceEvent::UserJoined { user_id, display_name, user_color } => {
                if user_id.is_empty() || display_name.is_empty() || user_color.is_empty() {
                    Err("UserJoined requires non-empty user_id, display_name, and user_color".to_string())
                } else {
                    Ok(())
                }
            },
            DeviceEvent::UserLeft { user_id, display_name, user_color } => {
                if user_id.is_empty() || display_name.is_empty() || user_color.is_empty() {
                    Err("UserLeft requires non-empty user_id, display_name, and user_color".to_string())
                } else {
                    Ok(())
                }
            },
            // Device event validations
            DeviceEvent::DeviceVariableUpdate { device_id, variable_name, .. } => {
                if device_id.is_empty() || variable_name.is_empty() {
                    Err("DeviceVariableUpdate requires non-empty device_id and variable_name".to_string())
                } else {
                    Ok(())
                }
            },
            DeviceEvent::DeviceStartOptions { device_id, .. } => {
                if device_id.is_empty() {
                    Err("DeviceStartOptions requires non-empty device_id".to_string())
                } else {
                    Ok(())
                }
            },
            DeviceEvent::DeviceChangeableVariables { device_id, .. } => {
                if device_id.is_empty() {
                    Err("DeviceChangeableVariables requires non-empty device_id".to_string())
                } else {
                    Ok(())
                }
            },
            DeviceEvent::DeviceUdpBroadcast { device_id, .. } => {
                if device_id.is_empty() {
                    Err("DeviceUdpBroadcast requires non-empty device_id".to_string())
                } else {
                    Ok(())
                }
            },
            DeviceEvent::DeviceConnectionStatus { device_id, .. } => {
                if device_id.is_empty() {
                    Err("DeviceConnectionStatus requires non-empty device_id".to_string())
                } else {
                    Ok(())
                }
            },
            DeviceEvent::DeviceDeviceInfo { device_id, .. } => {
                if device_id.is_empty() {
                    Err("DeviceDeviceInfo requires non-empty device_id".to_string())
                } else {
                    Ok(())
                }
            },
            DeviceEvent::DeviceDiscovered { device_id, device_ip, .. } => {
                if device_id.is_empty() || device_ip.is_empty() {
                    Err("DeviceDiscovered requires non-empty device_id and device_ip".to_string())
                } else {
                    Ok(())
                }
            },
        }
    }
}

