use std::sync::Mutex;

use serde::Serialize;

use crate::session::{DisplayWorkerHandle, DisplayWorkerSlot};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum SessionPhase {
    NoDevice,
    Ready,
    Flashing,
    Done,
    Error,
}

impl SessionPhase {
    fn kind(self) -> &'static str {
        match self {
            SessionPhase::NoDevice => "NoDevice",
            SessionPhase::Ready => "Ready",
            SessionPhase::Flashing => "Flashing",
            SessionPhase::Done => "Done",
            SessionPhase::Error => "Error",
        }
    }

    fn default_title(self) -> &'static str {
        match self {
            SessionPhase::NoDevice => "未连接",
            SessionPhase::Ready => "准备就绪",
            SessionPhase::Flashing => "写入中",
            SessionPhase::Done => "写入完成",
            SessionPhase::Error => "无法写入",
        }
    }

    fn button_enabled(self) -> bool {
        matches!(self, SessionPhase::Ready | SessionPhase::Done)
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SessionSnapshot {
    pub phase: SessionPhase,
    pub port_name: Option<String>,
    pub vid_pid: Option<String>,
    pub serial: Option<String>,
    pub button_enabled: bool,
    pub title: String,
}

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

impl From<SessionSnapshot> for UiDeviceStatus {
    fn from(snapshot: SessionSnapshot) -> Self {
        Self {
            kind: snapshot.phase.kind().to_string(),
            title: snapshot.title,
            port_name: snapshot.port_name,
            vid_pid: snapshot.vid_pid,
            serial: snapshot.serial,
            button_enabled: snapshot.button_enabled,
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

#[derive(Debug, Clone)]
struct SessionInner {
    phase: SessionPhase,
    port_name: Option<String>,
    vid_pid: Option<String>,
    serial: Option<String>,
    title: String,
}

impl Default for SessionInner {
    fn default() -> Self {
        Self {
            phase: SessionPhase::NoDevice,
            port_name: None,
            vid_pid: None,
            serial: None,
            title: SessionPhase::NoDevice.default_title().to_string(),
        }
    }
}

impl SessionInner {
    fn set_device(
        &mut self,
        phase: SessionPhase,
        port_name: String,
        vid_pid: Option<String>,
        serial: Option<String>,
    ) {
        self.phase = phase;
        self.port_name = Some(port_name);
        self.vid_pid = vid_pid;
        self.serial = serial;
        self.title = phase.default_title().to_string();
    }

    fn snapshot(&self) -> SessionSnapshot {
        SessionSnapshot {
            phase: self.phase,
            port_name: self.port_name.clone(),
            vid_pid: self.vid_pid.clone(),
            serial: self.serial.clone(),
            button_enabled: self.phase.button_enabled(),
            title: self.title.clone(),
        }
    }
}

pub struct AppState {
    session: Mutex<SessionInner>,
    display_worker: Mutex<DisplayWorkerSlot<DisplayWorkerHandle>>,
    logs: Mutex<Vec<LogEntry>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            session: Mutex::new(SessionInner::default()),
            display_worker: Mutex::new(DisplayWorkerSlot::default()),
            logs: Mutex::new(Vec::new()),
        }
    }
}

impl AppState {
    pub fn snapshot(&self) -> SessionSnapshot {
        self.session.lock().unwrap().snapshot()
    }

    pub fn set_ready(&self, port: String, vid_pid: Option<String>, serial: Option<String>) {
        self.session
            .lock()
            .unwrap()
            .set_device(SessionPhase::Ready, port, vid_pid, serial);
    }

    pub fn set_flashing(&self, port: String, vid_pid: Option<String>, serial: Option<String>) {
        self.session
            .lock()
            .unwrap()
            .set_device(SessionPhase::Flashing, port, vid_pid, serial);
    }

    pub fn set_done(&self, port: String, vid_pid: Option<String>, serial: Option<String>) {
        self.session
            .lock()
            .unwrap()
            .set_device(SessionPhase::Done, port, vid_pid, serial);
    }

    pub fn set_error(
        &self,
        port: Option<String>,
        vid_pid: Option<String>,
        serial: Option<String>,
        title: impl Into<String>,
    ) {
        let mut session = self.session.lock().unwrap();
        session.phase = SessionPhase::Error;
        session.port_name = port;
        session.vid_pid = vid_pid;
        session.serial = serial;
        session.title = title.into();
    }

    pub fn set_ready_device(&self, port: String, vid_pid: Option<String>, serial: Option<String>) {
        self.set_ready(port, vid_pid, serial);
    }

    pub fn clear_device(&self) {
        *self.session.lock().unwrap() = SessionInner::default();
    }

    pub fn selected_port(&self) -> Option<String> {
        self.session.lock().unwrap().port_name.clone()
    }

    pub(crate) fn with_display_worker_slot<R>(
        &self,
        f: impl FnOnce(&mut DisplayWorkerSlot<DisplayWorkerHandle>) -> R,
    ) -> R {
        let mut slot = self.display_worker.lock().unwrap();
        f(&mut slot)
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
    fn ready_snapshot_enables_flash_button() {
        let state = AppState::default();

        state.set_ready(
            "COM4".to_string(),
            Some("1A86:FE0C".to_string()),
            Some("0123456789".to_string()),
        );

        let snapshot = state.snapshot();
        assert_eq!(snapshot.phase, SessionPhase::Ready);
        assert_eq!(snapshot.port_name.as_deref(), Some("COM4"));
        assert_eq!(snapshot.vid_pid.as_deref(), Some("1A86:FE0C"));
        assert_eq!(snapshot.serial.as_deref(), Some("0123456789"));
        assert!(snapshot.button_enabled);
        assert_eq!(UiDeviceStatus::from(snapshot).kind, "Ready");
    }

    #[test]
    fn flashing_snapshot_disables_flash_button() {
        let state = AppState::default();

        state.set_flashing(
            "COM4".to_string(),
            Some("1A86:FE0C".to_string()),
            Some("0123456789".to_string()),
        );

        let snapshot = state.snapshot();
        assert_eq!(snapshot.phase, SessionPhase::Flashing);
        assert_eq!(snapshot.port_name.as_deref(), Some("COM4"));
        assert!(!snapshot.button_enabled);
        assert_eq!(UiDeviceStatus::from(snapshot).kind, "Flashing");
    }

    #[test]
    fn done_snapshot_allows_reflash() {
        let state = AppState::default();

        state.set_done(
            "COM4".to_string(),
            Some("1A86:FE0C".to_string()),
            Some("0123456789".to_string()),
        );

        let snapshot = state.snapshot();
        assert_eq!(snapshot.phase, SessionPhase::Done);
        assert!(snapshot.button_enabled);
        assert_eq!(UiDeviceStatus::from(snapshot).kind, "Done");
    }

    #[test]
    fn clear_device_stops_selected_port() {
        let state = AppState::default();
        state.set_ready(
            "COM4".to_string(),
            Some("1A86:FE0C".to_string()),
            Some("0123456789".to_string()),
        );

        state.clear_device();

        let snapshot = state.snapshot();
        assert_eq!(snapshot.phase, SessionPhase::NoDevice);
        assert_eq!(snapshot.port_name, None);
        assert_eq!(state.selected_port(), None);
        assert!(!snapshot.button_enabled);
    }

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
