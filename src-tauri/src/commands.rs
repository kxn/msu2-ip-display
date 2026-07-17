use tauri::{AppHandle, Emitter, State};

use crate::app_state::{AppState, UiDeviceStatus};
use crate::assets::{embedded_assets, fixed_flash_plan, validate_plan};
use crate::device::{handshake, open_serial_port, scan_candidates};
use crate::errors::{AppError, UiError};
use crate::flasher::{flash_images, preview_pages, FlashProgress};

fn emit_device_status(app: &AppHandle, status: &UiDeviceStatus) {
    let _ = app.emit("device-status-changed", status);
}

fn flash_failure(app: &AppHandle, state: &AppState, err: &AppError) -> UiError {
    let ui = err.to_ui_error();
    state.push_log(ui.message.clone(), ui.detail.clone());
    let _ = app.emit("flash-failed", &ui);
    ui
}

#[tauri::command]
pub fn scan_devices(app: AppHandle, state: State<'_, AppState>) -> Result<UiDeviceStatus, UiError> {
    let candidates = scan_candidates().map_err(|err| err.to_ui_error())?;
    let Some(candidate) = candidates.first() else {
        state.clear_device();
        state.push_log("未连接", "No candidate ports");
        let status = UiDeviceStatus::no_device();
        emit_device_status(&app, &status);
        return Ok(status);
    };

    let mut port = match open_serial_port(&candidate.port_name) {
        Ok(port) => port,
        Err(err) => {
            state.clear_device();
            state.push_log(err.user_message(), err.detail());
            let status = UiDeviceStatus::no_device();
            emit_device_status(&app, &status);
            return Err(err.to_ui_error());
        }
    };

    if let Err(err) = handshake(&mut port) {
        state.clear_device();
        state.push_log(err.user_message(), err.detail());
        let status = UiDeviceStatus::no_device();
        emit_device_status(&app, &status);
        return Err(err.to_ui_error());
    }

    state.set_ready_device(
        candidate.port_name.clone(),
        candidate.vid_pid.clone(),
        candidate.serial.clone(),
    );
    state.push_log("设备就绪", format!("Ready on {}", candidate.port_name));

    let status = UiDeviceStatus::ready(
        &candidate.port_name,
        candidate.vid_pid.as_deref().unwrap_or(""),
        candidate.serial.as_deref().unwrap_or(""),
    );
    emit_device_status(&app, &status);
    Ok(status)
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

    if let Err(err) = flash_images(&mut port, &plan, |progress: FlashProgress| {
        let _ = app.emit("flash-progress", progress);
    }) {
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
