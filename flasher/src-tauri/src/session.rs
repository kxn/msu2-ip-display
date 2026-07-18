use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::app_state::{AppState, UiDeviceStatus};
use crate::device::{handshake, open_serial_port, PortIo, SerialPortIo};
use crate::errors::{AppError, AppResult};
use crate::screen_status::{keepalive_pixel, show_flash_done, show_waiting_to_flash};

const KEEPALIVE_INTERVAL: Duration = Duration::from_millis(800);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayWorkerMode {
    WaitingToFlash,
    FlashDone,
}

pub trait DisplayWorkerFactory {
    type Handle;

    fn start(&mut self, port_name: String, mode: DisplayWorkerMode) -> AppResult<Self::Handle>;
    fn stop(&mut self, handle: Self::Handle);
}

pub struct DisplayWorkerSlot<H> {
    active: Option<H>,
}

impl<H> Default for DisplayWorkerSlot<H> {
    fn default() -> Self {
        Self { active: None }
    }
}

impl<H> DisplayWorkerSlot<H> {
    pub fn has_active(&self) -> bool {
        self.active.is_some()
    }

    pub fn start<F>(
        &mut self,
        factory: &mut F,
        port_name: String,
        mode: DisplayWorkerMode,
    ) -> AppResult<()>
    where
        F: DisplayWorkerFactory<Handle = H>,
    {
        self.stop(factory);
        let handle = factory.start(port_name, mode)?;
        self.active = Some(handle);
        Ok(())
    }

    pub fn stop<F>(&mut self, factory: &mut F)
    where
        F: DisplayWorkerFactory<Handle = H>,
    {
        if let Some(handle) = self.active.take() {
            factory.stop(handle);
        }
    }

    pub fn stop_before_exclusive_use<F, R>(
        &mut self,
        factory: &mut F,
        exclusive_use: impl FnOnce() -> R,
    ) -> R
    where
        F: DisplayWorkerFactory<Handle = H>,
    {
        self.stop(factory);
        exclusive_use()
    }
}

pub struct DisplayWorkerHandle {
    stop_tx: Option<Sender<()>>,
    join: Option<JoinHandle<()>>,
}

impl DisplayWorkerHandle {
    fn new(stop_tx: Sender<()>, join: JoinHandle<()>) -> Self {
        Self {
            stop_tx: Some(stop_tx),
            join: Some(join),
        }
    }

    fn stop(mut self) {
        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(());
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

pub struct RealDisplayWorkerFactory {
    app: AppHandle,
}

impl RealDisplayWorkerFactory {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl DisplayWorkerFactory for RealDisplayWorkerFactory {
    type Handle = DisplayWorkerHandle;

    fn start(&mut self, port_name: String, mode: DisplayWorkerMode) -> AppResult<Self::Handle> {
        start_display_worker(self.app.clone(), port_name, mode)
    }

    fn stop(&mut self, handle: Self::Handle) {
        stop_display_worker(Some(handle));
    }
}

pub fn show_display_mode<P: PortIo>(port: &mut P, mode: DisplayWorkerMode) -> AppResult<()> {
    match mode {
        DisplayWorkerMode::WaitingToFlash => show_waiting_to_flash(port),
        DisplayWorkerMode::FlashDone => show_flash_done(port),
    }
}

pub fn start_display_worker(
    app: AppHandle,
    port_name: String,
    mode: DisplayWorkerMode,
) -> AppResult<DisplayWorkerHandle> {
    let mut port = open_serial_port(&port_name)?;
    handshake(&mut port)?;
    show_display_mode(&mut port, mode)?;

    let (stop_tx, stop_rx) = mpsc::channel();
    let join = spawn_keepalive_loop(app, port_name, port, stop_rx);
    Ok(DisplayWorkerHandle::new(stop_tx, join))
}

pub fn stop_display_worker(handle: Option<DisplayWorkerHandle>) {
    if let Some(handle) = handle {
        handle.stop();
    }
}

pub fn start_active_display_worker(
    app: &AppHandle,
    state: &AppState,
    port_name: String,
    mode: DisplayWorkerMode,
) -> AppResult<()> {
    let mut factory = RealDisplayWorkerFactory::new(app.clone());
    state.with_display_worker_slot(|slot| slot.start(&mut factory, port_name, mode))
}

pub fn stop_active_display_worker(app: &AppHandle, state: &AppState) {
    let mut factory = RealDisplayWorkerFactory::new(app.clone());
    state.with_display_worker_slot(|slot| slot.stop(&mut factory));
}

fn spawn_keepalive_loop(
    app: AppHandle,
    port_name: String,
    mut port: SerialPortIo,
    stop_rx: Receiver<()>,
) -> JoinHandle<()> {
    thread::spawn(move || loop {
        match stop_rx.recv_timeout(KEEPALIVE_INTERVAL) {
            Ok(_) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Err(err) = keepalive_pixel(&mut port) {
                    handle_keepalive_failure(&app, &port_name, err);
                    break;
                }
            }
        }
    })
}

fn handle_keepalive_failure(app: &AppHandle, port_name: &str, err: AppError) {
    let state = app.state::<AppState>();
    state.push_log("设备已断开", err.detail());

    if state.selected_port().as_deref() == Some(port_name) {
        state.clear_device();
        let status = UiDeviceStatus::from(state.snapshot());
        let _ = app.emit("device-status-changed", status);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::AppResult;

    #[derive(Default)]
    struct FakeWorkerFactory {
        next_handle: usize,
        started: Vec<(String, DisplayWorkerMode)>,
        stopped: Vec<usize>,
    }

    impl DisplayWorkerFactory for FakeWorkerFactory {
        type Handle = usize;

        fn start(&mut self, port_name: String, mode: DisplayWorkerMode) -> AppResult<Self::Handle> {
            self.started.push((port_name, mode));
            self.next_handle += 1;
            Ok(self.next_handle)
        }

        fn stop(&mut self, handle: Self::Handle) {
            self.stopped.push(handle);
        }
    }

    #[test]
    fn starting_ready_worker_records_waiting_mode() {
        let mut slot = DisplayWorkerSlot::default();
        let mut factory = FakeWorkerFactory::default();

        slot.start(
            &mut factory,
            "COM4".to_string(),
            DisplayWorkerMode::WaitingToFlash,
        )
        .unwrap();

        assert!(slot.has_active());
        assert_eq!(
            factory.started,
            vec![("COM4".to_string(), DisplayWorkerMode::WaitingToFlash)]
        );
        assert!(factory.stopped.is_empty());
    }

    #[test]
    fn starting_done_worker_records_done_mode() {
        let mut slot = DisplayWorkerSlot::default();
        let mut factory = FakeWorkerFactory::default();

        slot.start(
            &mut factory,
            "COM4".to_string(),
            DisplayWorkerMode::FlashDone,
        )
        .unwrap();

        assert_eq!(
            factory.started,
            vec![("COM4".to_string(), DisplayWorkerMode::FlashDone)]
        );
    }

    #[test]
    fn stopping_worker_is_idempotent() {
        let mut slot = DisplayWorkerSlot::default();
        let mut factory = FakeWorkerFactory::default();

        slot.start(
            &mut factory,
            "COM4".to_string(),
            DisplayWorkerMode::WaitingToFlash,
        )
        .unwrap();
        slot.stop(&mut factory);
        slot.stop(&mut factory);

        assert!(!slot.has_active());
        assert_eq!(factory.stopped, vec![1]);
    }

    #[test]
    fn start_flash_stops_existing_worker_before_opening_flash_port() {
        let mut slot = DisplayWorkerSlot::default();
        let mut factory = FakeWorkerFactory::default();
        let mut events = Vec::new();

        slot.start(
            &mut factory,
            "COM4".to_string(),
            DisplayWorkerMode::WaitingToFlash,
        )
        .unwrap();
        slot.stop_before_exclusive_use(&mut factory, || events.push("open-flash"));

        assert_eq!(factory.stopped, vec![1]);
        assert_eq!(events, vec!["open-flash"]);
    }
}
