use std::sync::Mutex;

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct UiDeviceStatus {
    pub kind: String,
    pub title: String,
    pub port_name: Option<String>,
    pub vid_pid: Option<String>,
    pub serial: Option<String>,
    pub button_enabled: bool,
}

impl UiDeviceStatus {
    pub fn no_device() -> Self {
        Self {
            kind: "NoDevice".to_string(),
            title: "未连接".to_string(),
            port_name: None,
            vid_pid: None,
            serial: None,
            button_enabled: false,
        }
    }

    pub fn ready(port_name: &str, vid_pid: &str, serial: &str) -> Self {
        Self {
            kind: "Ready".to_string(),
            title: "准备就绪".to_string(),
            port_name: Some(port_name.to_string()),
            vid_pid: optional_field(vid_pid),
            serial: optional_field(serial),
            button_enabled: true,
        }
    }
}

fn optional_field(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub user_message: String,
    pub detail: String,
}

#[derive(Default)]
pub struct AppState {
    pub selected_port: Mutex<Option<String>>,
    pub selected_vid_pid: Mutex<Option<String>>,
    pub selected_serial: Mutex<Option<String>>,
    logs: Mutex<Vec<LogEntry>>,
}

impl AppState {
    pub fn set_ready_device(&self, port: String, vid_pid: Option<String>, serial: Option<String>) {
        *self.selected_port.lock().unwrap() = Some(port);
        *self.selected_vid_pid.lock().unwrap() = vid_pid;
        *self.selected_serial.lock().unwrap() = serial;
    }

    pub fn clear_device(&self) {
        *self.selected_port.lock().unwrap() = None;
        *self.selected_vid_pid.lock().unwrap() = None;
        *self.selected_serial.lock().unwrap() = None;
    }

    pub fn selected_port(&self) -> Option<String> {
        self.selected_port.lock().unwrap().clone()
    }

    pub fn push_log(&self, user_message: impl Into<String>, detail: impl Into<String>) {
        let mut logs = self.logs.lock().unwrap();
        logs.push(LogEntry {
            user_message: user_message.into(),
            detail: detail.into(),
        });
        if logs.len() > 500 {
            logs.remove(0);
        }
    }

    pub fn last_user_log(&self) -> Option<String> {
        self.logs
            .lock()
            .unwrap()
            .last()
            .map(|entry| entry.user_message.clone())
    }

    pub fn copy_log(&self) -> String {
        self.logs
            .lock()
            .unwrap()
            .iter()
            .map(|entry| format!("{} | {}", entry.user_message, entry.detail))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_status_uses_short_user_text() {
        let status = UiDeviceStatus::ready("COM4", "1A86:FE0C", "0123456789");
        assert_eq!(status.kind, "Ready");
        assert_eq!(status.title, "准备就绪");
        assert_eq!(status.port_name.as_deref(), Some("COM4"));
    }

    #[test]
    fn copied_log_contains_detail_but_ui_line_stays_short() {
        let state = AppState::default();
        state.push_log("设备就绪", "Handshake reply: 00 4D 53 4E 43 4E");
        assert_eq!(state.last_user_log().unwrap(), "设备就绪");
        assert!(state.copy_log().contains("Handshake reply"));
    }
}
