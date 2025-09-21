use std::fs::OpenOptions;
use std::io::Write;
use chrono::Utc;

pub struct DebugLogger;

impl DebugLogger {
    const LOG_FILE: &'static str = "debug_events.log";

    pub fn log_event(category: &str, message: &str) {
        let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let log_entry = format!("[{}] {}: {}\n", timestamp, category, message);

        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(Self::LOG_FILE) {
            let _ = file.write_all(log_entry.as_bytes());
            let _ = file.flush();
        }
    }

    pub fn log_event_forwarding_task_start(device_id: &str) {
        Self::log_event("EVENT_FORWARDING_TASK", &format!("STARTED for device {}", device_id));
    }

    pub fn log_event_forwarding_task_end(device_id: &str, event_count: usize) {
        Self::log_event("EVENT_FORWARDING_TASK", &format!("TERMINATED for device {} after {} events", device_id, event_count));
    }

    pub fn log_event_forwarding_task_receive(device_id: &str, event_count: usize, event: &str) {
        Self::log_event("EVENT_FORWARDING_TASK", &format!("RECEIVED event #{} from device {}: {}", event_count, device_id, event));
    }

    pub fn log_event_forwarding_task_send_success(device_id: &str, event_count: usize) {
        Self::log_event("EVENT_FORWARDING_TASK", &format!("SENT event #{} from device {} to manager", event_count, device_id));
    }

    pub fn log_event_forwarding_task_send_fail(device_id: &str, event_count: usize, error: &str) {
        Self::log_event("EVENT_FORWARDING_TASK", &format!("SEND_FAILED event #{} from device {} to manager: {}", event_count, device_id, error));
    }

    pub fn log_manager_event_processor_start() {
        Self::log_event("MANAGER_EVENT_PROCESSOR", "STARTED");
    }

    pub fn log_manager_event_processor_receive(event_count: usize, event: &str) {
        Self::log_event("MANAGER_EVENT_PROCESSOR", &format!("RECEIVED event #{}: {}", event_count, event));
    }

    pub fn log_manager_event_processor_end(event_count: usize) {
        Self::log_event("MANAGER_EVENT_PROCESSOR", &format!("TERMINATED after {} events", event_count));
    }

    pub fn log_device_add(device_id: &str) {
        Self::log_event("DEVICE_MANAGEMENT", &format!("ADD_DEVICE called for {}", device_id));
    }

    pub fn log_device_already_exists(device_id: &str) {
        Self::log_event("DEVICE_MANAGEMENT", &format!("DEVICE_ALREADY_EXISTS {}", device_id));
    }

    pub fn log_esp32_connection_event_send(device_id: &str, is_closed: bool, success: bool, error: Option<&str>) {
        let status = if success { "SUCCESS" } else { "FAILED" };
        let details = if let Some(err) = error {
            format!(" error: {}", err)
        } else {
            String::new()
        };
        Self::log_event("ESP32_CONNECTION", &format!("EVENT_SEND {} for device {} (channel_closed: {}){}", status, device_id, is_closed, details));
    }

    pub fn log_tcp_command_send(device_id: &str, command: &str, tcp_available: bool) {
        Self::log_event("TCP_COMMAND", &format!("SENDING command '{}' to device {} - TCP_AVAILABLE: {}", command, device_id, tcp_available));
    }

    pub fn log_tcp_command_success(device_id: &str, command: &str) {
        Self::log_event("TCP_COMMAND", &format!("SUCCESS sent command '{}' to device {}", command, device_id));
    }

    pub fn log_tcp_command_failed(device_id: &str, command: &str, error: &str) {
        Self::log_event("TCP_COMMAND", &format!("FAILED to send command '{}' to device {}: {}", command, device_id, error));
    }

    pub fn log_tcp_connection_status(device_id: &str, status: &str, details: &str) {
        Self::log_event("TCP_CONNECTION", &format!("STATUS for device {}: {} - {}", device_id, status, details));
    }

    pub fn log_tcp_reconnect_attempt(device_id: &str, reason: &str) {
        Self::log_event("TCP_RECONNECT", &format!("ATTEMPTING reconnect for device {} - reason: {}", device_id, reason));
    }

    pub fn log_tcp_reconnect_result(device_id: &str, success: bool, error: Option<&str>) {
        let status = if success { "SUCCESS" } else { "FAILED" };
        let details = if let Some(err) = error {
            format!(" - error: {}", err)
        } else {
            String::new()
        };
        Self::log_event("TCP_RECONNECT", &format!("RESULT for device {}: {}{}", device_id, status, details));
    }

    pub fn clear_log() {
        let _ = std::fs::remove_file(Self::LOG_FILE);
    }

    pub fn log_reset_attempt(device_id: &str, attempt_number: u32) {
        Self::log_to_temp_log(&format!("RESET_ATTEMPT_{}: Device {} - Reset command initiated", attempt_number, device_id));
    }

    pub fn log_reset_success(device_id: &str, attempt_number: u32) {
        Self::log_to_temp_log(&format!("RESET_SUCCESS_{}: Device {} - Reset command sent successfully", attempt_number, device_id));
    }

    pub fn log_reset_failure(device_id: &str, attempt_number: u32, error: &str) {
        Self::log_to_temp_log(&format!("RESET_FAILURE_{}: Device {} - Reset failed: {}", attempt_number, device_id, error));
    }

    pub fn log_connection_drop(device_id: &str, reason: &str) {
        Self::log_to_temp_log(&format!("CONNECTION_DROP: Device {} - Connection dropped: {}", device_id, reason));
    }

    pub fn log_device_manager_state(device_id: &str, state: &str) {
        Self::log_to_temp_log(&format!("DEVICE_MANAGER_STATE: Device {} - {}", device_id, state));
    }

    fn log_to_temp_log(message: &str) {
        let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let log_entry = format!("[{}] {}\n", timestamp, message);

        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open("templog.log") {
            let _ = file.write_all(log_entry.as_bytes());
            let _ = file.flush();
        }
    }
}