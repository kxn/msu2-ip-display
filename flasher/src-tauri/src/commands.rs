use tauri::{AppHandle, Emitter, State};

use crate::app_state::{AppState, UiDeviceStatus};
use crate::assets::{embedded_assets, fixed_flash_plan, validate_plan};
use crate::device::{handshake, open_serial_port, scan_candidates, DeviceInfo};
use crate::errors::{AppError, AppResult, UiError};
use crate::flasher::{flash_images_with_screen_status, preview_pages, FlashProgress};

fn emit_device_status(app: &AppHandle, status: &UiDeviceStatus) {
    let _ = app.emit("device-status-changed", status);
}

fn flash_failure(app: &AppHandle, state: &AppState, err: &AppError) -> UiError {
    let ui = err.to_ui_error();
    state.push_log(ui.message.clone(), ui.detail.clone());
    let _ = app.emit("flash-failed", &ui);
    ui
}

fn scan_no_device(
    state: &AppState,
    user_message: impl Into<String>,
    detail: impl Into<String>,
) -> UiDeviceStatus {
    state.clear_device();
    state.push_log(user_message, detail);
    UiDeviceStatus::no_device()
}

fn candidate_debug_line(candidate: &DeviceInfo) -> String {
    let mut parts = vec![candidate.port_name.clone()];
    if let Some(vid_pid) = candidate.vid_pid.as_deref() {
        parts.push(format!("VID:PID {vid_pid}"));
    }
    if let Some(serial) = candidate.serial.as_deref() {
        parts.push(format!("ID {serial}"));
    }
    parts.join(", ")
}

fn scan_devices_with<P, FOpen, FHandshake>(
    state: &AppState,
    candidates: AppResult<Vec<DeviceInfo>>,
    mut open_port: FOpen,
    mut run_handshake: FHandshake,
) -> UiDeviceStatus
where
    FOpen: FnMut(&str) -> AppResult<P>,
    FHandshake: FnMut(&mut P) -> AppResult<()>,
{
    let candidates = match candidates {
        Ok(candidates) => candidates,
        Err(err) => return scan_no_device(state, err.user_message(), err.detail()),
    };

    if candidates.is_empty() {
        return scan_no_device(state, "未连接", "No candidate ports");
    }

    let mut failures = Vec::new();

    for candidate in candidates {
        let mut port = match open_port(&candidate.port_name) {
            Ok(port) => port,
            Err(err) => {
                failures.push(format!(
                    "{} -> {}",
                    candidate_debug_line(&candidate),
                    err.detail()
                ));
                continue;
            }
        };

        if let Err(err) = run_handshake(&mut port) {
            failures.push(format!(
                "{} -> {}",
                candidate_debug_line(&candidate),
                err.detail()
            ));
            continue;
        }

        state.set_ready_device(
            candidate.port_name.clone(),
            candidate.vid_pid.clone(),
            candidate.serial.clone(),
        );
        state.push_log(
            "设备就绪",
            format!("Ready on {}", candidate_debug_line(&candidate)),
        );

        return UiDeviceStatus::ready(
            &candidate.port_name,
            candidate.vid_pid.as_deref().unwrap_or(""),
            candidate.serial.as_deref().unwrap_or(""),
        );
    }

    scan_no_device(state, "未连接", failures.join("; "))
}

#[tauri::command]
pub fn scan_devices(app: AppHandle, state: State<'_, AppState>) -> UiDeviceStatus {
    let status = scan_devices_with(&state, scan_candidates(), open_serial_port, handshake);
    emit_device_status(&app, &status);
    status
}

#[tauri::command]
pub fn start_flash(app: AppHandle, state: State<'_, AppState>) -> Result<(), UiError> {
    let port_name = match state.selected_port() {
        Some(port_name) => port_name,
        None => return Err(flash_failure(&app, &state, &AppError::NoDevice)),
    };
    let assets = embedded_assets();
    let plan = fixed_flash_plan(&assets);
    if let Err(err) = validate_plan(&plan) {
        let app_error = AppError::Asset(err.to_string());
        return Err(flash_failure(&app, &state, &app_error));
    }

    let mut port = match open_serial_port(&port_name) {
        Ok(port) => port,
        Err(err) => return Err(flash_failure(&app, &state, &err)),
    };
    if let Err(err) = handshake(&mut port) {
        return Err(flash_failure(&app, &state, &err));
    }

    state.push_log("写入中", format!("Starting flash on {port_name}"));

    if let Err(err) =
        flash_images_with_screen_status(&mut port, &plan, |progress: FlashProgress| {
            let _ = app.emit("flash-progress", progress);
        })
    {
        return Err(flash_failure(&app, &state, &err));
    }

    if let Err(err) = preview_pages(&mut port) {
        return Err(flash_failure(&app, &state, &err));
    }

    state.push_log("写入完成", "DONE");
    let _ = app.emit("flash-finished", "写入完成");
    Ok(())
}

#[tauri::command]
pub fn copy_log(state: State<'_, AppState>) -> String {
    state.copy_log()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TestPort {
        port_name: String,
    }

    #[test]
    fn scan_failure_status_clears_stale_device_and_returns_no_device() {
        let state = AppState::default();
        state.set_ready_device(
            "COM9".to_string(),
            Some("1A86:FE0C".to_string()),
            Some("serial-123".to_string()),
        );

        let status = scan_no_device(&state, "未连接", "Detailed scan failure");

        assert_eq!(status, UiDeviceStatus::no_device());
        assert_eq!(state.selected_port(), None);
        assert_eq!(state.last_user_log().as_deref(), Some("未连接"));
        assert!(state.copy_log().contains("Detailed scan failure"));
    }

    #[test]
    fn scan_tries_later_candidate_after_first_handshake_failure() {
        let state = AppState::default();
        let opened = RefCell::new(Vec::new());

        let status = scan_devices_with(
            &state,
            Ok(vec![
                DeviceInfo {
                    port_name: "COM5".to_string(),
                    vid_pid: Some("1A86:FE0C".to_string()),
                    product: Some("CH32x035".to_string()),
                    serial: Some("first".to_string()),
                },
                DeviceInfo {
                    port_name: "COM6".to_string(),
                    vid_pid: Some("1A86:FE0C".to_string()),
                    product: Some("CH32x035".to_string()),
                    serial: Some("second".to_string()),
                },
            ]),
            |port_name| {
                opened.borrow_mut().push(port_name.to_string());
                Ok(TestPort {
                    port_name: port_name.to_string(),
                })
            },
            |port| {
                if port.port_name == "COM5" {
                    Err(AppError::HandshakeFailed)
                } else {
                    Ok(())
                }
            },
        );

        assert_eq!(
            opened.into_inner(),
            vec!["COM5".to_string(), "COM6".to_string()]
        );
        assert_eq!(status.port_name.as_deref(), Some("COM6"));
        assert_eq!(status.vid_pid.as_deref(), Some("1A86:FE0C"));
        assert_eq!(status.serial.as_deref(), Some("second"));
        assert_eq!(state.selected_port(), Some("COM6".to_string()));
    }

    #[test]
    fn scan_returns_no_device_when_all_candidates_fail() {
        let state = AppState::default();
        state.set_ready_device(
            "COM9".to_string(),
            Some("1A86:FE0C".to_string()),
            Some("serial-123".to_string()),
        );

        let status = scan_devices_with(
            &state,
            Ok(vec![
                DeviceInfo {
                    port_name: "COM5".to_string(),
                    vid_pid: Some("1A86:FE0C".to_string()),
                    product: Some("CH32x035".to_string()),
                    serial: Some("first".to_string()),
                },
                DeviceInfo {
                    port_name: "COM6".to_string(),
                    vid_pid: Some("1A86:FE0C".to_string()),
                    product: Some("CH32x035".to_string()),
                    serial: Some("second".to_string()),
                },
            ]),
            |port_name| -> AppResult<TestPort> {
                match port_name {
                    "COM5" => Err(AppError::PortBusy(port_name.to_string())),
                    "COM6" => Ok(TestPort {
                        port_name: port_name.to_string(),
                    }),
                    _ => unreachable!(),
                }
            },
            |port| {
                assert_eq!(port.port_name, "COM6");
                Err(AppError::HandshakeFailed)
            },
        );

        assert_eq!(status, UiDeviceStatus::no_device());
        assert_eq!(state.selected_port(), None);
        assert!(state
            .copy_log()
            .contains("Handshake did not return expected MSNCN bytes"));
    }
}
