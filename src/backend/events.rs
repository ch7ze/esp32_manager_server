// ============================================================================
// ESP32 DEVICE EVENTS - Event Definitions for Client-Server Communication
// ============================================================================

use serde::{Deserialize, Serialize};
use serde_json;

// ============================================================================
// CLIENT-SERVER COMMUNICATION MESSAGES
// ============================================================================

/// WebSocket messages sent from Client to Server
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command")]
pub enum ClientMessage {
    #[serde(rename = "registerForDevice")]
    RegisterForDevice { 
        #[serde(rename = "deviceId")]
        device_id: String 
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
// ESP32 DEVICE EVENT DEFINITIONS - Compatible with Frontend EventBus
// ============================================================================

/// ESP32 device events for device management and control
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event")]
pub enum DeviceEvent {
    #[serde(rename = "deviceCommand")]
    DeviceCommand {
        command: String,
        parameters: Option<serde_json::Value>,
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
}

// ============================================================================
// SHAPE DEFINITIONS - Compatible with Frontend Shape Classes
// ============================================================================

/// Shape data structure matching frontend implementation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shape {
    #[serde(rename = "type")]
    pub shape_type: ShapeType,
    pub id: String,
    pub data: ShapeData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShapeType {
    Line,
    Rectangle, 
    Circle,
    Triangle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShapeData {
    // Common properties for all shapes
    #[serde(rename = "zOrder")]
    pub z_order: i32,
    #[serde(rename = "bgColor")]
    pub bg_color: Option<String>, // "transparent", "ff0000", etc.
    #[serde(rename = "fgColor")]
    pub fg_color: String, // "000000", etc.
    
    // Shape-specific properties (optional based on shape type)
    pub from: Option<Point>,
    pub to: Option<Point>,
    pub center: Option<Point>,
    pub radius: Option<f64>,
    pub p1: Option<Point>,
    pub p2: Option<Point>,
    pub p3: Option<Point>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
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
    pub fn device_command(command: String, parameters: Option<serde_json::Value>) -> Self {
        DeviceEvent::DeviceCommand { command, parameters }
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
}

impl Shape {
    pub fn new_line(id: String, from: Point, to: Point, fg_color: String, z_order: i32) -> Self {
        Shape {
            shape_type: ShapeType::Line,
            id,
            data: ShapeData {
                z_order,
                bg_color: None, // Lines don't have background
                fg_color,
                from: Some(from),
                to: Some(to),
                center: None,
                radius: None,
                p1: None,
                p2: None,
                p3: None,
            },
        }
    }
    
    pub fn new_rectangle(id: String, from: Point, to: Point, fg_color: String, bg_color: Option<String>, z_order: i32) -> Self {
        Shape {
            shape_type: ShapeType::Rectangle,
            id,
            data: ShapeData {
                z_order,
                bg_color,
                fg_color,
                from: Some(from),
                to: Some(to),
                center: None,
                radius: None,
                p1: None,
                p2: None,
                p3: None,
            },
        }
    }
    
    pub fn new_circle(id: String, center: Point, radius: f64, fg_color: String, bg_color: Option<String>, z_order: i32) -> Self {
        Shape {
            shape_type: ShapeType::Circle,
            id,
            data: ShapeData {
                z_order,
                bg_color,
                fg_color,
                from: None,
                to: None,
                center: Some(center),
                radius: Some(radius),
                p1: None,
                p2: None,
                p3: None,
            },
        }
    }
    
    pub fn new_triangle(id: String, p1: Point, p2: Point, p3: Point, fg_color: String, bg_color: Option<String>, z_order: i32) -> Self {
        Shape {
            shape_type: ShapeType::Triangle,
            id,
            data: ShapeData {
                z_order,
                bg_color,
                fg_color,
                from: None,
                to: None,
                center: None,
                radius: None,
                p1: Some(p1),
                p2: Some(p2),
                p3: Some(p3),
            },
        }
    }
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Point { x, y }
    }
}

// ============================================================================
// VALIDATION HELPERS
// ============================================================================

impl DeviceEvent {
    /// Validate that the event has all required data for its type
    pub fn validate(&self) -> Result<(), String> {
        match self {
            DeviceEvent::DeviceCommand { command, .. } => {
                if command.is_empty() {
                    Err("DeviceCommand requires non-empty command".to_string())
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
        }
    }
}

impl Shape {
    /// Validate that the shape has all required data for its type
    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("Shape must have non-empty id".to_string());
        }
        
        match self.shape_type {
            ShapeType::Line => {
                if self.data.from.is_none() || self.data.to.is_none() {
                    Err("Line shape requires from and to points".to_string())
                } else {
                    Ok(())
                }
            },
            ShapeType::Rectangle => {
                if self.data.from.is_none() || self.data.to.is_none() {
                    Err("Rectangle shape requires from and to points".to_string())
                } else {
                    Ok(())
                }
            },
            ShapeType::Circle => {
                if self.data.center.is_none() || self.data.radius.is_none() {
                    Err("Circle shape requires center and radius".to_string())
                } else if self.data.radius.unwrap() <= 0.0 {
                    Err("Circle radius must be positive".to_string())
                } else {
                    Ok(())
                }
            },
            ShapeType::Triangle => {
                if self.data.p1.is_none() || self.data.p2.is_none() || self.data.p3.is_none() {
                    Err("Triangle shape requires p1, p2, and p3 points".to_string())
                } else {
                    Ok(())
                }
            },
        }
    }
}