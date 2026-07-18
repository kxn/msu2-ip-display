use tauri::{AppHandle, Emitter, Manager, State};

use crate::app_state::{AppState, SessionPhase, UiDeviceStatus};
use crate::assets::{embedded_assets, fixed_flash_plan, validate_plan};
use crate::device::{handshake, open_serial_port, scan_candidates, DeviceInfo};
use crate::errors::{AppError, AppResult, UiError};
use crate::flasher::{flash_images_with_screen_status, FlashProgress};
use crate::session::{start_active_display_worker, stop_active_display_worker, DisplayWorkerMode};

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

fn scan_devices_with<FStart, FStop>(
    state: &AppState,
    candidates: AppResult<Vec<DeviceInfo>>,
    mut start_waiting_worker: FStart,
    mut stop_display_worker: FStop,
) -> UiDeviceStatus
where
    FStart: FnMut(&DeviceInfo) -> AppResult<()>,
    FStop: FnMut(),
{
    let snapshot = state.snapshot();
    if snapshot.phase == SessionPhase::Flashing {
        return UiDeviceStatus::from(snapshot);
    }

    let candidates = match candidates {
        Ok(candidates) => candidates,
        Err(err) => {
            stop_display_worker();
            return scan_no_device(state, err.user_message(), err.detail());
        }
    };

    if candidates.is_empty() {
        stop_display_worker();
        return scan_no_device(state, "未连接", "No candidate ports");
    }

    let snapshot = state.snapshot();
    if let Some(selected_port) = snapshot.port_name.as_deref() {
        let selected_still_present = candidates
            .iter()
            .any(|candidate| candidate.port_name == selected_port);
        if selected_still_present {
            match snapshot.phase {
                SessionPhase::Ready | SessionPhase::Done | SessionPhase::Flashing => {
                    return UiDeviceStatus::from(snapshot);
                }
                SessionPhase::NoDevice | SessionPhase::Error => {}
            }
        } else if matches!(snapshot.phase, SessionPhase::Ready | SessionPhase::Done) {
            stop_display_worker();
            return scan_no_device(
                state,
                "未连接",
                format!("Selected port {selected_port} disappeared"),
            );
        }
    }

    let mut failures = Vec::new();

    for candidate in candidates {
        if let Err(err) = start_waiting_worker(&candidate) {
            failures.push(format!(
                "{} -> {}",
                candidate_debug_line(&candidate),
                err.detail()
            ));
            continue;
        }

        state.set_ready(
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

fn queue_flash_start<FStop, FDispatch>(
    state: &AppState,
    mut stop_display_worker: FStop,
    dispatch: FDispatch,
) -> AppResult<()>
where
    FStop: FnMut(),
    FDispatch: FnOnce(String),
{
    let snapshot = state.snapshot();
    if snapshot.phase == SessionPhase::Flashing {
        return Err(AppError::FlashAlreadyRunning);
    }
    if !matches!(snapshot.phase, SessionPhase::Ready | SessionPhase::Done) {
        return Err(AppError::NoDevice);
    }

    let port_name = snapshot.port_name.ok_or(AppError::NoDevice)?;
    stop_display_worker();
    state.set_flashing(
        port_name.clone(),
        snapshot.vid_pid.clone(),
        snapshot.serial.clone(),
    );
    dispatch(port_name);
    Ok(())
}

fn complete_flash_success_with<FStartDone>(
    state: &AppState,
    port_name: &str,
    start_done_worker: FStartDone,
) -> AppResult<()>
where
    FStartDone: FnOnce(&str, DisplayWorkerMode) -> AppResult<()>,
{
    let snapshot = state.snapshot();
    start_done_worker(port_name, DisplayWorkerMode::FlashDone)?;
    state.set_done(
        port_name.to_string(),
        snapshot.vid_pid.clone(),
        snapshot.serial.clone(),
    );
    state.push_log("写入完成", "DONE");
    Ok(())
}

fn complete_flash_success(app: &AppHandle, state: &AppState, port_name: &str) -> AppResult<()> {
    complete_flash_success_with(state, port_name, |port_name, mode| {
        start_active_display_worker(app, state, port_name.to_string(), mode)
    })?;
    let status = UiDeviceStatus::from(state.snapshot());
    let _ = app.emit("device-status-changed", &status);
    let _ = app.emit("flash-finished", "写入完成");
    Ok(())
}

fn mark_flash_error(state: &AppState, err: &AppError) {
    let snapshot = state.snapshot();
    state.set_error(
        snapshot.port_name,
        snapshot.vid_pid,
        snapshot.serial,
        err.user_message(),
    );
}

#[tauri::command]
pub fn scan_devices(app: AppHandle, state: State<'_, AppState>) -> UiDeviceStatus {
    let status = scan_devices_with(
        &state,
        scan_candidates(),
        |candidate| {
            start_active_display_worker(
                &app,
                &state,
                candidate.port_name.clone(),
                DisplayWorkerMode::WaitingToFlash,
            )
        },
        || stop_active_display_worker(&app, &state),
    );
    emit_device_status(&app, &status);
    status
}

#[tauri::command]
pub fn start_flash(app: AppHandle, state: State<'_, AppState>) -> Result<(), UiError> {
    queue_flash_start(
        &state,
        || stop_active_display_worker(&app, &state),
        |port_name| {
            let app = app.clone();
            tauri::async_runtime::spawn_blocking(move || run_flash_job(app, port_name));
        },
    )
    .map_err(|err| flash_failure(&app, &state, &err))
}

fn run_flash_job(app: AppHandle, port_name: String) {
    let state = app.state::<AppState>();
    match run_flash_sequence(&app, &state, &port_name) {
        Ok(()) => {
            if let Err(err) = complete_flash_success(&app, &state, &port_name) {
                mark_flash_error(&state, &err);
                flash_failure(&app, &state, &err);
            }
        }
        Err(err) => {
            mark_flash_error(&state, &err);
            flash_failure(&app, &state, &err);
        }
    }
}

fn run_flash_sequence(app: &AppHandle, state: &AppState, port_name: &str) -> AppResult<()> {
    let assets = embedded_assets();
    let plan = fixed_flash_plan(&assets);
    if let Err(err) = validate_plan(&plan) {
        return Err(AppError::Asset(err.to_string()));
    }

    let mut port = open_serial_port(port_name)?;
    handshake(&mut port)?;

    state.push_log("写入中", format!("Starting flash on {port_name}"));

    flash_images_with_screen_status(&mut port, &plan, |progress: FlashProgress| {
        let _ = app.emit("flash-progress", progress);
    })?;

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

    #[test]
    fn scan_failure_status_clears_stale_device_and_returns_no_device() {
        let state = AppState::default();
        state.set_ready(
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
    fn scan_tries_later_candidate_after_first_worker_start_failure() {
        let state = AppState::default();
        let started = RefCell::new(Vec::new());
        let stopped = RefCell::new(0);

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
            |candidate| {
                started.borrow_mut().push(candidate.port_name.clone());
                if candidate.port_name == "COM5" {
                    Err(AppError::HandshakeFailed)
                } else {
                    Ok(())
                }
            },
            || *stopped.borrow_mut() += 1,
        );

        assert_eq!(
            started.into_inner(),
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
        let stopped = RefCell::new(0);

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
            |candidate| -> AppResult<()> {
                match candidate.port_name.as_str() {
                    "COM5" => Err(AppError::PortBusy(candidate.port_name.clone())),
                    "COM6" => Err(AppError::HandshakeFailed),
                    _ => unreachable!(),
                }
            },
            || *stopped.borrow_mut() += 1,
        );

        assert_eq!(status, UiDeviceStatus::no_device());
        assert_eq!(state.selected_port(), None);
        assert_eq!(*stopped.borrow(), 0);
        assert!(state
            .copy_log()
            .contains("Handshake did not return expected MSNCN bytes"));
    }

    #[test]
    fn scan_starts_waiting_worker_for_new_device() {
        let state = AppState::default();
        let started = RefCell::new(Vec::new());
        let stopped = RefCell::new(0);

        let status = scan_devices_with(
            &state,
            Ok(vec![DeviceInfo {
                port_name: "COM4".to_string(),
                vid_pid: Some("1A86:FE0C".to_string()),
                product: Some("CH32x035".to_string()),
                serial: Some("serial-123".to_string()),
            }]),
            |candidate| {
                started.borrow_mut().push((
                    candidate.port_name.clone(),
                    DisplayWorkerMode::WaitingToFlash,
                ));
                Ok(())
            },
            || *stopped.borrow_mut() += 1,
        );

        assert_eq!(status.port_name.as_deref(), Some("COM4"));
        assert_eq!(
            started.into_inner(),
            vec![("COM4".to_string(), DisplayWorkerMode::WaitingToFlash)]
        );
        assert_eq!(*stopped.borrow(), 0);
    }

    #[test]
    fn scan_does_not_reopen_existing_ready_device() {
        let state = AppState::default();
        state.set_ready(
            "COM4".to_string(),
            Some("1A86:FE0C".to_string()),
            Some("serial-123".to_string()),
        );
        let started = RefCell::new(Vec::new());
        let stopped = RefCell::new(0);

        let status = scan_devices_with(
            &state,
            Ok(vec![DeviceInfo {
                port_name: "COM4".to_string(),
                vid_pid: Some("1A86:FE0C".to_string()),
                product: Some("CH32x035".to_string()),
                serial: Some("serial-123".to_string()),
            }]),
            |candidate| {
                started.borrow_mut().push(candidate.port_name.clone());
                Ok(())
            },
            || *stopped.borrow_mut() += 1,
        );

        assert_eq!(status.port_name.as_deref(), Some("COM4"));
        assert_eq!(status.kind, "Ready");
        assert_eq!(started.into_inner(), Vec::<String>::new());
        assert_eq!(*stopped.borrow(), 0);
    }

    #[test]
    fn scan_does_not_reopen_existing_done_device() {
        let state = AppState::default();
        state.set_done(
            "COM4".to_string(),
            Some("1A86:FE0C".to_string()),
            Some("serial-123".to_string()),
        );
        let started = RefCell::new(Vec::new());
        let stopped = RefCell::new(0);

        let status = scan_devices_with(
            &state,
            Ok(vec![DeviceInfo {
                port_name: "COM4".to_string(),
                vid_pid: Some("1A86:FE0C".to_string()),
                product: Some("CH32x035".to_string()),
                serial: Some("serial-123".to_string()),
            }]),
            |candidate| {
                started.borrow_mut().push(candidate.port_name.clone());
                Ok(())
            },
            || *stopped.borrow_mut() += 1,
        );

        assert_eq!(status.port_name.as_deref(), Some("COM4"));
        assert_eq!(status.kind, "Done");
        assert_eq!(started.into_inner(), Vec::<String>::new());
        assert_eq!(*stopped.borrow(), 0);
    }

    #[test]
    fn scan_clears_state_when_selected_port_disappears() {
        let state = AppState::default();
        state.set_ready(
            "COM4".to_string(),
            Some("1A86:FE0C".to_string()),
            Some("serial-123".to_string()),
        );
        let stopped = RefCell::new(0);

        let status = scan_devices_with(
            &state,
            Ok(vec![DeviceInfo {
                port_name: "COM5".to_string(),
                vid_pid: Some("1A86:FE0C".to_string()),
                product: Some("CH32x035".to_string()),
                serial: Some("other".to_string()),
            }]),
            |_| Ok(()),
            || *stopped.borrow_mut() += 1,
        );

        assert_eq!(status.kind, "NoDevice");
        assert_eq!(state.selected_port(), None);
        assert_eq!(*stopped.borrow(), 1);
    }

    #[test]
    fn scan_recovers_error_to_ready_when_device_remains() {
        let state = AppState::default();
        state.set_error(
            Some("COM4".to_string()),
            Some("1A86:FE0C".to_string()),
            Some("serial-123".to_string()),
            "无法写入",
        );
        let started = RefCell::new(Vec::new());
        let stopped = RefCell::new(0);

        let status = scan_devices_with(
            &state,
            Ok(vec![DeviceInfo {
                port_name: "COM4".to_string(),
                vid_pid: Some("1A86:FE0C".to_string()),
                product: Some("CH32x035".to_string()),
                serial: Some("serial-123".to_string()),
            }]),
            |candidate| {
                started.borrow_mut().push(candidate.port_name.clone());
                Ok(())
            },
            || *stopped.borrow_mut() += 1,
        );

        assert_eq!(status.kind, "Ready");
        assert_eq!(started.into_inner(), vec!["COM4".to_string()]);
        assert_eq!(*stopped.borrow(), 0);
    }

    #[test]
    fn scan_keeps_flashing_state_without_touching_serial() {
        let state = AppState::default();
        state.set_flashing(
            "COM4".to_string(),
            Some("1A86:FE0C".to_string()),
            Some("serial-123".to_string()),
        );
        let started = RefCell::new(Vec::new());
        let stopped = RefCell::new(0);

        let status = scan_devices_with(
            &state,
            Err(AppError::Io(
                "enumeration unavailable during flash".to_string(),
            )),
            |candidate| {
                started.borrow_mut().push(candidate.port_name.clone());
                Ok(())
            },
            || *stopped.borrow_mut() += 1,
        );

        assert_eq!(status.kind, "Flashing");
        assert_eq!(status.port_name.as_deref(), Some("COM4"));
        assert_eq!(started.into_inner(), Vec::<String>::new());
        assert_eq!(*stopped.borrow(), 0);
    }

    #[test]
    fn start_flash_rejects_no_device() {
        let state = AppState::default();
        let stopped = RefCell::new(0);
        let dispatched = RefCell::new(Vec::new());

        let err = queue_flash_start(
            &state,
            || *stopped.borrow_mut() += 1,
            |port_name| dispatched.borrow_mut().push(port_name),
        )
        .unwrap_err();

        assert!(matches!(err, AppError::NoDevice));
        assert_eq!(*stopped.borrow(), 0);
        assert!(dispatched.borrow().is_empty());
    }

    #[test]
    fn start_flash_rejects_already_flashing() {
        let state = AppState::default();
        state.set_flashing(
            "COM4".to_string(),
            Some("1A86:FE0C".to_string()),
            Some("serial-123".to_string()),
        );
        let stopped = RefCell::new(0);
        let dispatched = RefCell::new(Vec::new());

        let err = queue_flash_start(
            &state,
            || *stopped.borrow_mut() += 1,
            |port_name| dispatched.borrow_mut().push(port_name),
        )
        .unwrap_err();

        assert!(matches!(err, AppError::FlashAlreadyRunning));
        assert_eq!(*stopped.borrow(), 0);
        assert!(dispatched.borrow().is_empty());
    }

    #[test]
    fn start_flash_stops_ready_keepalive_before_flash() {
        let state = AppState::default();
        state.set_ready(
            "COM4".to_string(),
            Some("1A86:FE0C".to_string()),
            Some("serial-123".to_string()),
        );
        let events = RefCell::new(Vec::new());

        queue_flash_start(
            &state,
            || events.borrow_mut().push("stop".to_string()),
            |port_name| events.borrow_mut().push(format!("flash:{port_name}")),
        )
        .unwrap();

        assert_eq!(
            events.into_inner(),
            vec!["stop".to_string(), "flash:COM4".to_string()]
        );
        let snapshot = state.snapshot();
        assert_eq!(snapshot.phase, SessionPhase::Flashing);
        assert!(!snapshot.button_enabled);
    }

    #[test]
    fn successful_flash_starts_done_keepalive() {
        let state = AppState::default();
        state.set_flashing(
            "COM4".to_string(),
            Some("1A86:FE0C".to_string()),
            Some("serial-123".to_string()),
        );
        let started = RefCell::new(Vec::new());

        complete_flash_success_with(&state, "COM4", |port_name, mode| {
            started.borrow_mut().push((port_name.to_string(), mode));
            Ok(())
        })
        .unwrap();

        assert_eq!(
            started.into_inner(),
            vec![("COM4".to_string(), DisplayWorkerMode::FlashDone)]
        );
        let snapshot = state.snapshot();
        assert_eq!(snapshot.phase, SessionPhase::Done);
        assert!(snapshot.button_enabled);
    }

    #[test]
    fn failed_flash_marks_error_and_does_not_start_done_keepalive() {
        let state = AppState::default();
        state.set_flashing(
            "COM4".to_string(),
            Some("1A86:FE0C".to_string()),
            Some("serial-123".to_string()),
        );

        mark_flash_error(&state, &AppError::HandshakeFailed);

        let snapshot = state.snapshot();
        assert_eq!(snapshot.phase, SessionPhase::Error);
        assert_eq!(snapshot.port_name.as_deref(), Some("COM4"));
        assert!(!snapshot.button_enabled);
    }
}
