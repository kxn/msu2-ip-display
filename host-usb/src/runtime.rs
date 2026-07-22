use std::io;
use std::time::{Duration, Instant};

use crate::cli::{DisplayMode, ResourceMode, RunOptions};
use crate::daemon::{Daemon, DaemonAction, DaemonEvent, DaemonState};
use crate::device_scan::TtyDevice;
use crate::display::{DisplayRenderer, WireWrite};
use crate::ip_detect::NetworkSnapshot;
use crate::serial::{classify_io_error, SerialErrorKind, TimeoutCounter};

const KEEPALIVE_INTERVAL: Duration = Duration::from_millis(800);
const NETWORK_SNAPSHOT_STALE_INTERVAL: Duration = Duration::from_secs(2);
const STATUS_PAGE_REFRESH_INTERVAL: Duration = Duration::from_secs(2);
const CONNECT_RETRY_MIN_INTERVAL: Duration = Duration::from_millis(500);
const CONNECT_RETRY_MAX_INTERVAL: Duration = Duration::from_secs(2);

pub trait RuntimeIo {
    fn scan_devices(&mut self) -> io::Result<Vec<TtyDevice>>;
    fn connect(&mut self, device: &TtyDevice) -> io::Result<()>;
    fn disconnect(&mut self);
    fn network_snapshot(&mut self) -> io::Result<Option<NetworkSnapshot>>;
    fn send_writes(&mut self, writes: &[WireWrite]) -> io::Result<()>;
    fn sleep(&mut self, duration: Duration);
    fn now(&self) -> Instant;
}

pub struct Runtime<T> {
    daemon: Daemon,
    io: T,
    display_mode: DisplayMode,
    resource_mode: ResourceMode,
    connected: bool,
    last_keepalive: Instant,
    last_network_snapshot: Instant,
    last_status_page_refresh: Instant,
    timeout_counter: TimeoutCounter,
    pending_display_action: Option<DaemonAction>,
    next_connect_at: Option<Instant>,
    connect_backoff: Duration,
    keepalive_pixel: KeepalivePixel,
}

impl<T: RuntimeIo> Runtime<T> {
    pub fn new(options: RunOptions, io: T) -> Self {
        let now = io.now();
        let display_mode = options.show.clone();
        let resource_mode = options.resources;
        Self {
            daemon: Daemon::new(options),
            io,
            display_mode,
            resource_mode,
            connected: false,
            last_keepalive: now,
            last_network_snapshot: now,
            last_status_page_refresh: now,
            timeout_counter: TimeoutCounter::new(3),
            pending_display_action: None,
            next_connect_at: None,
            connect_backoff: CONNECT_RETRY_MIN_INTERVAL,
            keepalive_pixel: KeepalivePixel::Black,
        }
    }

    pub fn tick(&mut self) -> io::Result<()> {
        if !self.connected {
            let devices = self.io.scan_devices()?;
            crate::logging::debug(&format!("scan found {} target tty(s)", devices.len()));
            let Some(device) = devices.first() else {
                if self.daemon.state != DaemonState::Listening {
                    self.disconnect_device();
                }
                self.reset_connect_retry();
                self.io.sleep(Duration::from_millis(500));
                return Ok(());
            };

            if self.daemon.state == DaemonState::Listening {
                for action in self.daemon.handle_event(DaemonEvent::DeviceCandidateFound) {
                    self.apply_action(&action)?;
                }
            }

            if let Some(next_connect_at) = self.next_connect_at {
                let now = self.io.now();
                if now < next_connect_at {
                    crate::logging::debug("connect retry backoff active");
                    self.io.sleep(Duration::from_millis(500));
                    return Ok(());
                }
            }

            if let Err(err) = self.io.connect(device) {
                self.handle_connect_failure("connect", err);
                return Ok(());
            }
            self.connected = true;
            self.reset_connect_retry();
            self.timeout_counter.record_success();
            let now = self.io.now();
            self.last_keepalive = now;
            self.last_network_snapshot = now;
            self.last_status_page_refresh = now;

            for action in self.daemon.handle_event(DaemonEvent::HandshakeOk) {
                if let Err(err) = self.apply_daemon_action(action, DisplayActionLog::StateChange) {
                    self.handle_connect_failure("show pending", err);
                    return Ok(());
                }
            }
        }

        if let Some(action) = self.pending_display_action.clone() {
            if let Err(err) = self.apply_action(&action) {
                return self.handle_runtime_error("display retry", err, true);
            }
            log_display_action(&action, DisplayActionLog::Refresh);
            self.pending_display_action = None;
            let now = self.io.now();
            self.last_keepalive = now;
            self.last_status_page_refresh = now;
        }

        let now = self.io.now();

        let snapshot = match self.io.network_snapshot() {
            Ok(Some(snapshot)) => {
                self.last_network_snapshot = now;
                Some(snapshot)
            }
            Ok(None) => {
                crate::logging::debug("network snapshot worker has no new result");
                self.stale_network_snapshot(now)
            }
            Err(err) => {
                crate::logging::debug(&format!("network snapshot unavailable: {err}"));
                self.stale_network_snapshot(now)
            }
        };

        if let Some(snapshot) = snapshot {
            crate::logging::debug(&format!(
                "network snapshot: {} addresses, {} default routes",
                snapshot.addresses.len(),
                snapshot
                    .routes
                    .iter()
                    .filter(|route| route.is_default)
                    .count()
            ));
            {
                for action in self
                    .daemon
                    .handle_event(DaemonEvent::NetworkSnapshot { snapshot, now })
                {
                    if let Err(err) =
                        self.apply_daemon_action(action, DisplayActionLog::StateChange)
                    {
                        return self.handle_runtime_error("display update", err, true);
                    }
                }
            }
        }

        if let Some(action) = self.status_page_refresh_action(now) {
            if let Err(err) = self.apply_daemon_action(action, DisplayActionLog::Refresh) {
                return self.handle_runtime_error("status page refresh", err, true);
            }
            self.last_status_page_refresh = now;
        }

        if now.duration_since(self.last_keepalive) >= KEEPALIVE_INTERVAL {
            let writes = match self.keepalive_pixel {
                KeepalivePixel::Black => DisplayRenderer::keepalive(),
                KeepalivePixel::White => DisplayRenderer::keepalive_white(),
            };
            if let Err(err) = self.io.send_writes(&writes) {
                return self.handle_runtime_error("keepalive", err, true);
            }
            crate::logging::debug("keepalive ok");
            self.timeout_counter.record_success();
            self.last_keepalive = now;
        }

        self.io.sleep(Duration::from_millis(500));
        Ok(())
    }

    fn apply_daemon_action(
        &mut self,
        action: DaemonAction,
        log: DisplayActionLog,
    ) -> io::Result<()> {
        let is_display_action = matches!(
            action,
            DaemonAction::ShowPending | DaemonAction::ShowDhcpFailed | DaemonAction::ShowIp(_)
        );
        if is_display_action {
            self.pending_display_action = Some(action.clone());
        }
        self.apply_action(&action)?;
        if is_display_action {
            log_display_action(&action, log);
            self.pending_display_action = None;
            let now = self.io.now();
            self.last_keepalive = now;
            self.last_status_page_refresh = now;
        }
        Ok(())
    }

    fn apply_action(&mut self, action: &DaemonAction) -> io::Result<()> {
        match action {
            DaemonAction::OpenDevice => Ok(()),
            DaemonAction::CloseDevice => {
                self.io.disconnect();
                self.connected = false;
                self.keepalive_pixel = KeepalivePixel::Black;
                Ok(())
            }
            DaemonAction::ShowPending => {
                let writes = match self.resource_mode {
                    ResourceMode::Flashed => DisplayRenderer::pending(),
                    ResourceMode::Unflashed => DisplayRenderer::pending_runtime(),
                };
                self.io.send_writes(&writes)?;
                self.keepalive_pixel = KeepalivePixel::Black;
                self.timeout_counter.record_success();
                Ok(())
            }
            DaemonAction::ShowDhcpFailed => {
                let writes = match self.resource_mode {
                    ResourceMode::Flashed => DisplayRenderer::dhcp_failed(),
                    ResourceMode::Unflashed => DisplayRenderer::dhcp_failed_runtime(),
                };
                self.io.send_writes(&writes)?;
                self.keepalive_pixel = KeepalivePixel::Black;
                self.timeout_counter.record_success();
                Ok(())
            }
            DaemonAction::ShowIp(ip) => {
                match &self.display_mode {
                    DisplayMode::Text => {
                        self.io.send_writes(&DisplayRenderer::ip(*ip))?;
                        self.keepalive_pixel = KeepalivePixel::Black;
                    }
                    DisplayMode::Qr { template } => {
                        let writes = match DisplayRenderer::qr(*ip, template) {
                            Ok(writes) => writes,
                            Err(err) => {
                                crate::logging::warn(&format!(
                                    "failed to render QR display for {ip}: {err}"
                                ));
                                return Ok(());
                            }
                        };
                        self.io.send_writes(&writes)?;
                        self.keepalive_pixel = KeepalivePixel::White;
                    }
                }
                self.timeout_counter.record_success();
                Ok(())
            }
        }
    }

    fn disconnect_device(&mut self) {
        self.pending_display_action = None;
        for action in self.daemon.handle_event(DaemonEvent::DeviceDisconnected) {
            let _ = self.apply_action(&action);
        }
        self.reset_connect_retry();
        self.timeout_counter.record_success();
    }

    #[cfg(any(target_os = "linux", test))]
    fn recover_after_unexpected_tick_error(&mut self, err: io::Error) {
        crate::logging::warn(&format!("runtime tick failed: {err}"));
        self.disconnect_device();
        self.io.sleep(Duration::from_millis(500));
    }

    fn handle_runtime_error(
        &mut self,
        action: &str,
        err: io::Error,
        counts_serial_timeout: bool,
    ) -> io::Result<()> {
        match classify_io_error(&err) {
            SerialErrorKind::Disconnected => {
                crate::logging::warn(&format!("{action} failed: {err}"));
                self.disconnect_device();
                self.io.sleep(Duration::from_millis(500));
                Ok(())
            }
            SerialErrorKind::Timeout => {
                if counts_serial_timeout && self.timeout_counter.record_timeout() {
                    crate::logging::warn(&format!("{action} timed out repeatedly: {err}"));
                    self.disconnect_device();
                } else if counts_serial_timeout {
                    crate::logging::debug(&format!("{action} timed out once: {err}"));
                } else {
                    crate::logging::debug(&format!("{action} timed out: {err}"));
                }
                self.io.sleep(Duration::from_millis(500));
                Ok(())
            }
            SerialErrorKind::Other => Err(err),
        }
    }

    fn handle_connect_failure(&mut self, action: &str, err: io::Error) {
        match classify_io_error(&err) {
            SerialErrorKind::Timeout => {
                crate::logging::debug(&format!("{action} timed out: {err}"));
            }
            SerialErrorKind::Disconnected => {
                crate::logging::warn(&format!("{action} disconnected: {err}"));
            }
            SerialErrorKind::Other => {
                crate::logging::warn(&format!("{action} failed: {err}"));
            }
        }
        self.io.disconnect();
        self.connected = false;
        self.pending_display_action = None;
        self.schedule_connect_retry();
        self.io.sleep(Duration::from_millis(500));
    }

    fn schedule_connect_retry(&mut self) {
        let delay = self.connect_backoff;
        self.next_connect_at = Some(self.io.now() + delay);
        self.connect_backoff = (delay * 2).min(CONNECT_RETRY_MAX_INTERVAL);
    }

    fn reset_connect_retry(&mut self) {
        self.next_connect_at = None;
        self.connect_backoff = CONNECT_RETRY_MIN_INTERVAL;
    }

    fn stale_network_snapshot(&self, now: Instant) -> Option<NetworkSnapshot> {
        if now.duration_since(self.last_network_snapshot) >= NETWORK_SNAPSHOT_STALE_INTERVAL {
            crate::logging::debug("using stale empty network snapshot");
            Some(NetworkSnapshot::default())
        } else {
            None
        }
    }

    fn status_page_refresh_action(&self, now: Instant) -> Option<DaemonAction> {
        if now.duration_since(self.last_status_page_refresh) < STATUS_PAGE_REFRESH_INTERVAL {
            return None;
        }

        match self.daemon.state {
            DaemonState::ConnectedPendingIp => Some(DaemonAction::ShowPending),
            DaemonState::ConnectedDhcpFailed => Some(DaemonAction::ShowDhcpFailed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DisplayActionLog {
    StateChange,
    Refresh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeepalivePixel {
    Black,
    White,
}

fn log_display_action(action: &DaemonAction, mode: DisplayActionLog) {
    let prefix = match mode {
        DisplayActionLog::StateChange => "display",
        DisplayActionLog::Refresh => "refresh display",
    };
    match action {
        DaemonAction::ShowPending => crate::logging::info(&format!("{prefix}: pending IP page")),
        DaemonAction::ShowDhcpFailed => {
            crate::logging::info(&format!("{prefix}: DHCP failed page"))
        }
        DaemonAction::ShowIp(ip) => crate::logging::info(&format!("{prefix}: IP {ip}")),
        DaemonAction::OpenDevice | DaemonAction::CloseDevice => {}
    }
}

#[cfg(target_os = "linux")]
pub struct LinuxRuntimeIo {
    session: Option<crate::serial::SerialSession>,
    network_slot: NetworkSnapshotSlot,
}

#[cfg(target_os = "linux")]
impl LinuxRuntimeIo {
    pub fn new() -> Self {
        let network_slot = new_network_snapshot_slot();
        spawn_network_snapshot_worker(network_slot.clone(), Duration::from_millis(500));
        Self {
            session: None,
            network_slot,
        }
    }
}

#[cfg(target_os = "linux")]
impl RuntimeIo for LinuxRuntimeIo {
    fn scan_devices(&mut self) -> io::Result<Vec<TtyDevice>> {
        crate::device_scan::scan_target_ttys()
    }

    fn connect(&mut self, device: &TtyDevice) -> io::Result<()> {
        self.session = None;
        crate::logging::info(&format!("opening screen serial {}", device.path.display()));
        std::thread::sleep(Duration::from_millis(300));
        let mut session = crate::serial::SerialSession::open(&device.path)?;
        crate::logging::debug("screen handshake start");
        session.handshake()?;
        crate::logging::info("screen handshake ok");
        self.session = Some(session);
        Ok(())
    }

    fn disconnect(&mut self) {
        self.session = None;
    }

    fn network_snapshot(&mut self) -> io::Result<Option<NetworkSnapshot>> {
        take_network_snapshot(&self.network_slot)
    }

    fn send_writes(&mut self, writes: &[WireWrite]) -> io::Result<()> {
        let Some(session) = self.session.as_mut() else {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "screen not connected",
            ));
        };
        session.send_writes(writes)
    }

    fn sleep(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }

    fn now(&self) -> Instant {
        Instant::now()
    }
}

#[cfg(any(target_os = "linux", test))]
#[derive(Clone)]
struct NetworkSnapshotSlot {
    inner: std::sync::Arc<std::sync::Mutex<Option<io::Result<NetworkSnapshot>>>>,
}

#[cfg(any(target_os = "linux", test))]
fn new_network_snapshot_slot() -> NetworkSnapshotSlot {
    NetworkSnapshotSlot {
        inner: std::sync::Arc::new(std::sync::Mutex::new(None)),
    }
}

#[cfg(any(target_os = "linux", test))]
fn store_network_snapshot(
    slot: &NetworkSnapshotSlot,
    result: io::Result<NetworkSnapshot>,
) -> io::Result<()> {
    let mut guard = slot
        .inner
        .lock()
        .map_err(|_| io::Error::other("network snapshot slot poisoned"))?;
    *guard = Some(result);
    Ok(())
}

#[cfg(any(target_os = "linux", test))]
fn take_network_snapshot(slot: &NetworkSnapshotSlot) -> io::Result<Option<NetworkSnapshot>> {
    let mut guard = slot
        .inner
        .lock()
        .map_err(|_| io::Error::other("network snapshot slot poisoned"))?;
    match guard.take() {
        Some(Ok(snapshot)) => Ok(Some(snapshot)),
        Some(Err(err)) => Err(err),
        None => Ok(None),
    }
}

#[cfg(target_os = "linux")]
fn spawn_network_snapshot_worker(slot: NetworkSnapshotSlot, interval: Duration) {
    std::thread::spawn(move || loop {
        let result = crate::ip_detect::collect_network_snapshot();
        if store_network_snapshot(&slot, result).is_err() {
            break;
        }
        std::thread::sleep(interval);
    });
}

#[cfg(target_os = "linux")]
pub fn run_forever(options: RunOptions) -> io::Result<()> {
    let mut runtime = Runtime::new(options, LinuxRuntimeIo::new());
    loop {
        if let Err(err) = runtime.tick() {
            runtime.recover_after_unexpected_tick_error(err);
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub fn run_forever(_options: RunOptions) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        crate::platform::unsupported_platform_message(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::collections::VecDeque;
    use std::net::Ipv4Addr;
    use std::path::PathBuf;

    use crate::cli::ResourceMode;
    use crate::display::DisplayRenderer;
    use crate::ip_detect::{AddressCandidate, Route};
    use crate::protocol::{
        load_lcd_address_packet, load_ram_show_packet, set_size_packet, set_xy_packet,
        SCREEN_HEIGHT, SCREEN_WIDTH,
    };

    #[derive(Default)]
    struct FakeIo {
        now: Cell<Option<Instant>>,
        devices: Vec<TtyDevice>,
        connect_results: VecDeque<io::Result<()>>,
        snapshot_results: VecDeque<io::Result<Option<NetworkSnapshot>>>,
        send_results: VecDeque<io::Result<()>>,
        events: Vec<String>,
    }

    impl FakeIo {
        fn set_now(&self, now: Instant) {
            self.now.set(Some(now));
        }
    }

    impl RuntimeIo for FakeIo {
        fn scan_devices(&mut self) -> io::Result<Vec<TtyDevice>> {
            self.events.push("scan".to_string());
            Ok(self.devices.clone())
        }

        fn connect(&mut self, device: &TtyDevice) -> io::Result<()> {
            self.events
                .push(format!("connect:{}", device.path.display()));
            self.connect_results.pop_front().unwrap_or(Ok(()))
        }

        fn disconnect(&mut self) {
            self.events.push("disconnect".to_string());
        }

        fn network_snapshot(&mut self) -> io::Result<Option<NetworkSnapshot>> {
            self.events.push("snapshot".to_string());
            self.snapshot_results
                .pop_front()
                .unwrap_or_else(|| Ok(Some(NetworkSnapshot::default())))
        }

        fn send_writes(&mut self, writes: &[WireWrite]) -> io::Result<()> {
            let marker = if writes == DisplayRenderer::pending() {
                "pending"
            } else if writes == DisplayRenderer::pending_runtime() {
                "pending_runtime"
            } else if writes == DisplayRenderer::dhcp_failed() {
                "dhcp_failed"
            } else if writes == DisplayRenderer::dhcp_failed_runtime() {
                "dhcp_failed_runtime"
            } else if is_qr_writes(writes) {
                "qr"
            } else if writes == DisplayRenderer::keepalive_white() {
                "keepalive_white"
            } else if writes
                .iter()
                .any(|write| write.bytes == load_ram_show_packet().to_vec())
            {
                "ip"
            } else {
                "other"
            };
            self.events.push(format!("writes:{marker}"));
            self.send_results.pop_front().unwrap_or(Ok(()))
        }

        fn sleep(&mut self, duration: Duration) {
            self.events.push(format!("sleep:{}", duration.as_millis()));
        }

        fn now(&self) -> Instant {
            self.now.get().unwrap_or_else(Instant::now)
        }
    }

    fn is_qr_writes(writes: &[WireWrite]) -> bool {
        writes.len() == 103
            && writes[0].bytes == set_xy_packet(0, 0).to_vec()
            && writes[1].bytes == set_size_packet(SCREEN_WIDTH, SCREEN_HEIGHT).to_vec()
            && writes[2].bytes == load_lcd_address_packet().to_vec()
    }

    fn options() -> RunOptions {
        RunOptions {
            interface: None,
            dhcp_fail_delay: Duration::from_secs(45),
            resources: ResourceMode::Flashed,
            debug: false,
            show: DisplayMode::Text,
        }
    }

    fn target_device() -> TtyDevice {
        TtyDevice {
            path: PathBuf::from("/dev/ttyACM0"),
            name: "ttyACM0".to_string(),
        }
    }

    fn ipv4_snapshot(address: [u8; 4]) -> NetworkSnapshot {
        NetworkSnapshot {
            addresses: vec![AddressCandidate {
                interface: "eth0".to_string(),
                address: Ipv4Addr::from(address),
                is_dynamic: true,
                is_up: true,
                is_lower_up: true,
            }],
            routes: vec![Route {
                interface: "eth0".to_string(),
                is_default: true,
            }],
        }
    }

    fn timeout_error() -> io::Error {
        io::Error::from_raw_os_error(libc::EAGAIN)
    }

    fn disconnected_error() -> io::Error {
        io::Error::from_raw_os_error(libc::EIO)
    }

    fn other_error() -> io::Error {
        io::Error::other("unexpected collector failure")
    }

    #[test]
    fn tick_connects_device_and_renders_pending_then_ip() {
        let start = Instant::now();
        let io = FakeIo {
            now: Cell::new(Some(start)),
            devices: vec![target_device()],
            snapshot_results: VecDeque::from([Ok(Some(ipv4_snapshot([192, 168, 1, 20])))]),
            ..FakeIo::default()
        };
        let mut runtime = Runtime::new(options(), io);

        runtime.tick().unwrap();

        assert_eq!(
            runtime.io.events,
            vec![
                "scan",
                "connect:/dev/ttyACM0",
                "writes:pending",
                "snapshot",
                "writes:ip",
                "sleep:500",
            ]
        );
    }

    #[test]
    fn unflashed_mode_direct_writes_pending_status_on_connect() {
        let start = Instant::now();
        let mut options = options();
        options.resources = ResourceMode::Unflashed;
        let io = FakeIo {
            now: Cell::new(Some(start)),
            devices: vec![target_device()],
            snapshot_results: VecDeque::from([Ok(Some(NetworkSnapshot::default()))]),
            ..FakeIo::default()
        };
        let mut runtime = Runtime::new(options, io);

        runtime.tick().unwrap();

        assert!(runtime
            .io
            .events
            .iter()
            .any(|event| event == "writes:pending_runtime"));
        assert!(!runtime
            .io
            .events
            .iter()
            .any(|event| event == "writes:pending"));
    }

    #[test]
    fn unflashed_mode_direct_writes_dhcp_failed_status() {
        let start = Instant::now();
        let mut options = options();
        options.resources = ResourceMode::Unflashed;
        let link_local = NetworkSnapshot {
            addresses: vec![AddressCandidate {
                interface: "eth0".to_string(),
                address: Ipv4Addr::new(169, 254, 1, 2),
                is_dynamic: true,
                is_up: true,
                is_lower_up: true,
            }],
            routes: vec![],
        };
        let io = FakeIo {
            now: Cell::new(Some(start)),
            devices: vec![target_device()],
            snapshot_results: VecDeque::from([Ok(Some(link_local.clone())), Ok(Some(link_local))]),
            ..FakeIo::default()
        };
        let mut runtime = Runtime::new(options, io);

        runtime.tick().unwrap();
        runtime.io.set_now(start + Duration::from_secs(45));
        runtime.tick().unwrap();

        assert!(runtime
            .io
            .events
            .iter()
            .any(|event| event == "writes:dhcp_failed_runtime"));
    }

    #[test]
    fn flashed_mode_keeps_page_based_pending_status() {
        let start = Instant::now();
        let io = FakeIo {
            now: Cell::new(Some(start)),
            devices: vec![target_device()],
            snapshot_results: VecDeque::from([Ok(Some(NetworkSnapshot::default()))]),
            ..FakeIo::default()
        };
        let mut runtime = Runtime::new(options(), io);

        runtime.tick().unwrap();

        assert!(runtime
            .io
            .events
            .iter()
            .any(|event| event == "writes:pending"));
        assert!(!runtime
            .io
            .events
            .iter()
            .any(|event| event == "writes:pending_runtime"));
    }

    #[test]
    fn qr_mode_renders_qr_instead_of_text_ip() {
        let start = Instant::now();
        let mut options = options();
        options.show = DisplayMode::Qr {
            template: "http://{ip}/".to_string(),
        };
        let io = FakeIo {
            now: Cell::new(Some(start)),
            devices: vec![target_device()],
            snapshot_results: VecDeque::from([Ok(Some(ipv4_snapshot([10, 0, 0, 5])))]),
            ..FakeIo::default()
        };
        let mut runtime = Runtime::new(options, io);

        runtime.tick().unwrap();

        assert!(runtime.io.events.iter().any(|event| event == "writes:qr"));
        assert!(!runtime.io.events.iter().any(|event| event == "writes:ip"));
    }

    #[test]
    fn qr_mode_keepalive_uses_white_pixel() {
        let start = Instant::now();
        let mut options = options();
        options.show = DisplayMode::Qr {
            template: "http://{ip}/".to_string(),
        };
        let snapshot = ipv4_snapshot([10, 0, 0, 5]);
        let io = FakeIo {
            now: Cell::new(Some(start)),
            devices: vec![target_device()],
            snapshot_results: VecDeque::from([Ok(Some(snapshot.clone())), Ok(Some(snapshot))]),
            ..FakeIo::default()
        };
        let mut runtime = Runtime::new(options, io);

        runtime.tick().unwrap();
        runtime.io.set_now(start + Duration::from_millis(800));
        runtime.tick().unwrap();

        assert!(runtime
            .io
            .events
            .iter()
            .any(|event| event == "writes:keepalive_white"));
    }

    #[test]
    fn keepalive_is_sent_within_one_second_after_displaying_ip() {
        let start = Instant::now();
        let snapshot = ipv4_snapshot([192, 168, 1, 20]);
        let io = FakeIo {
            now: Cell::new(Some(start)),
            devices: vec![target_device()],
            snapshot_results: VecDeque::from([Ok(Some(snapshot.clone())), Ok(Some(snapshot))]),
            ..FakeIo::default()
        };
        let mut runtime = Runtime::new(options(), io);

        runtime.tick().unwrap();
        runtime.io.set_now(start + Duration::from_millis(800));
        runtime.tick().unwrap();

        assert_eq!(
            runtime
                .io
                .events
                .iter()
                .filter(|event| event.as_str() == "writes:other")
                .count(),
            1
        );
    }

    #[test]
    fn tick_disconnects_after_three_consecutive_serial_timeouts() {
        let start = Instant::now();
        let io = FakeIo {
            now: Cell::new(Some(start)),
            devices: vec![target_device()],
            snapshot_results: VecDeque::from([
                Ok(Some(ipv4_snapshot([192, 168, 1, 20]))),
                Ok(Some(ipv4_snapshot([192, 168, 1, 20]))),
                Ok(Some(ipv4_snapshot([192, 168, 1, 20]))),
                Ok(Some(ipv4_snapshot([192, 168, 1, 20]))),
            ]),
            send_results: VecDeque::from([
                Ok(()),
                Ok(()),
                Err(timeout_error()),
                Err(timeout_error()),
                Err(timeout_error()),
            ]),
            ..FakeIo::default()
        };
        let mut runtime = Runtime::new(options(), io);

        runtime.tick().unwrap();
        runtime.io.set_now(start + Duration::from_secs(11));
        runtime.tick().unwrap();
        runtime.io.set_now(start + Duration::from_secs(12));
        runtime.tick().unwrap();
        runtime.io.set_now(start + Duration::from_secs(13));
        runtime.tick().unwrap();

        assert_eq!(
            runtime.io.events.last().map(String::as_str),
            Some("sleep:500")
        );
        assert_eq!(
            runtime
                .io
                .events
                .iter()
                .filter(|event| event.as_str() == "disconnect")
                .count(),
            1
        );
    }

    #[test]
    fn tick_recovers_to_scanning_after_disconnect_like_send_error() {
        let start = Instant::now();
        let io = FakeIo {
            now: Cell::new(Some(start)),
            devices: vec![target_device()],
            snapshot_results: VecDeque::from([
                Ok(Some(ipv4_snapshot([192, 168, 1, 20]))),
                Ok(Some(ipv4_snapshot([192, 168, 1, 20]))),
            ]),
            send_results: VecDeque::from([Ok(()), Ok(()), Err(disconnected_error())]),
            ..FakeIo::default()
        };
        let mut runtime = Runtime::new(options(), io);

        runtime.tick().unwrap();
        runtime.io.set_now(start + Duration::from_secs(11));
        runtime.tick().unwrap();
        runtime.io.set_now(start + Duration::from_secs(12));
        runtime.tick().unwrap();

        assert!(runtime
            .io
            .events
            .windows(2)
            .any(|window| window == ["disconnect", "sleep:500"]));
        assert!(runtime.io.events.iter().any(|event| event == "scan"));
    }

    #[test]
    fn serial_timeouts_remain_consecutive_across_network_snapshot_successes() {
        let start = Instant::now();
        let io = FakeIo {
            now: Cell::new(Some(start)),
            devices: vec![target_device()],
            snapshot_results: VecDeque::from([
                Ok(Some(ipv4_snapshot([192, 168, 1, 20]))),
                Ok(Some(ipv4_snapshot([192, 168, 1, 20]))),
                Ok(Some(ipv4_snapshot([192, 168, 1, 21]))),
                Ok(Some(ipv4_snapshot([192, 168, 1, 22]))),
            ]),
            send_results: VecDeque::from([
                Ok(()),
                Ok(()),
                Err(timeout_error()),
                Err(timeout_error()),
                Err(timeout_error()),
            ]),
            ..FakeIo::default()
        };
        let mut runtime = Runtime::new(options(), io);

        runtime.tick().unwrap();
        runtime.io.set_now(start + Duration::from_secs(11));
        runtime.tick().unwrap();
        runtime.io.set_now(start + Duration::from_secs(12));
        runtime.tick().unwrap();
        runtime.io.set_now(start + Duration::from_secs(13));
        runtime.tick().unwrap();

        assert_eq!(
            runtime
                .io
                .events
                .iter()
                .filter(|event| event.as_str() == "disconnect")
                .count(),
            1
        );
    }

    #[test]
    fn timed_out_display_update_is_retried_for_identical_snapshot() {
        let start = Instant::now();
        let snapshot = ipv4_snapshot([192, 168, 1, 20]);
        let io = FakeIo {
            now: Cell::new(Some(start)),
            devices: vec![target_device()],
            snapshot_results: VecDeque::from([Ok(Some(snapshot.clone())), Ok(Some(snapshot))]),
            send_results: VecDeque::from([Ok(()), Err(timeout_error()), Ok(())]),
            ..FakeIo::default()
        };
        let mut runtime = Runtime::new(options(), io);

        runtime.tick().unwrap();
        runtime.io.set_now(start + Duration::from_secs(1));
        runtime.tick().unwrap();

        assert_eq!(
            runtime
                .io
                .events
                .iter()
                .filter(|event| event.as_str() == "writes:ip")
                .count(),
            2
        );
    }

    #[test]
    fn network_snapshot_error_keeps_display_session_alive() {
        let start = Instant::now();
        let io = FakeIo {
            now: Cell::new(Some(start)),
            devices: vec![target_device()],
            snapshot_results: VecDeque::from([
                Err(other_error()),
                Ok(Some(ipv4_snapshot([192, 168, 1, 20]))),
            ]),
            ..FakeIo::default()
        };
        let mut runtime = Runtime::new(options(), io);

        runtime.tick().unwrap();
        runtime.io.set_now(start + Duration::from_millis(800));
        runtime.tick().unwrap();

        assert_eq!(
            runtime
                .io
                .events
                .iter()
                .filter(|event| event.as_str() == "writes:pending")
                .count(),
            1
        );
        assert!(runtime.io.events.iter().any(|event| event == "writes:ip"));
        assert!(!runtime.io.events.iter().any(|event| event == "disconnect"));
    }

    #[test]
    fn stale_network_snapshot_returns_display_to_pending() {
        let start = Instant::now();
        let io = FakeIo {
            now: Cell::new(Some(start)),
            devices: vec![target_device()],
            snapshot_results: VecDeque::from([
                Ok(Some(ipv4_snapshot([192, 168, 1, 20]))),
                Ok(None),
                Ok(None),
            ]),
            ..FakeIo::default()
        };
        let mut runtime = Runtime::new(options(), io);

        runtime.tick().unwrap();
        runtime.io.set_now(start + Duration::from_millis(1000));
        runtime.tick().unwrap();
        runtime.io.set_now(start + Duration::from_millis(2500));
        runtime.tick().unwrap();

        assert_eq!(
            runtime
                .io
                .events
                .iter()
                .filter(|event| event.as_str() == "writes:pending")
                .count(),
            2
        );
    }

    #[test]
    fn pending_page_is_periodically_refreshed_while_waiting_for_ip() {
        let start = Instant::now();
        let io = FakeIo {
            now: Cell::new(Some(start)),
            devices: vec![target_device()],
            snapshot_results: VecDeque::from([
                Ok(Some(NetworkSnapshot::default())),
                Ok(Some(NetworkSnapshot::default())),
            ]),
            ..FakeIo::default()
        };
        let mut runtime = Runtime::new(options(), io);

        runtime.tick().unwrap();
        runtime.io.set_now(start + Duration::from_millis(2100));
        runtime.tick().unwrap();

        assert_eq!(
            runtime
                .io
                .events
                .iter()
                .filter(|event| event.as_str() == "writes:pending")
                .count(),
            2
        );
    }

    #[test]
    fn failure_candidate_keeps_pending_page_visible_before_failure_delay() {
        let start = Instant::now();
        let link_local = NetworkSnapshot {
            addresses: vec![AddressCandidate {
                interface: "eth0".to_string(),
                address: Ipv4Addr::new(169, 254, 1, 2),
                is_dynamic: true,
                is_up: true,
                is_lower_up: true,
            }],
            routes: vec![],
        };
        let io = FakeIo {
            now: Cell::new(Some(start)),
            devices: vec![target_device()],
            snapshot_results: VecDeque::from([Ok(Some(link_local.clone())), Ok(Some(link_local))]),
            ..FakeIo::default()
        };
        let mut runtime = Runtime::new(options(), io);

        runtime.tick().unwrap();
        runtime.io.set_now(start + Duration::from_millis(2100));
        runtime.tick().unwrap();

        assert_eq!(
            runtime
                .io
                .events
                .iter()
                .filter(|event| event.as_str() == "writes:pending")
                .count(),
            2
        );
    }

    #[test]
    fn connect_failure_uses_backoff_before_retrying_same_device() {
        let start = Instant::now();
        let io = FakeIo {
            now: Cell::new(Some(start)),
            devices: vec![target_device()],
            connect_results: VecDeque::from([Err(timeout_error()), Ok(())]),
            ..FakeIo::default()
        };
        let mut runtime = Runtime::new(options(), io);

        runtime.tick().unwrap();
        runtime.io.set_now(start + Duration::from_millis(100));
        runtime.tick().unwrap();
        runtime.io.set_now(start + Duration::from_millis(500));
        runtime.tick().unwrap();

        assert_eq!(
            runtime
                .io
                .events
                .iter()
                .filter(|event| event.starts_with("connect:"))
                .count(),
            2
        );
        assert_eq!(
            runtime
                .io
                .events
                .iter()
                .position(|event| event.starts_with("connect:")),
            Some(1)
        );
    }

    #[test]
    fn pending_screen_failure_closes_half_open_session() {
        let start = Instant::now();
        let io = FakeIo {
            now: Cell::new(Some(start)),
            devices: vec![target_device()],
            send_results: VecDeque::from([Err(timeout_error())]),
            ..FakeIo::default()
        };
        let mut runtime = Runtime::new(options(), io);

        runtime.tick().unwrap();

        assert!(runtime.io.events.iter().any(|event| event == "disconnect"));
        assert!(!runtime.connected);
    }

    #[test]
    fn network_snapshot_slot_keeps_only_latest_snapshot() {
        let slot = new_network_snapshot_slot();

        store_network_snapshot(&slot, Ok(ipv4_snapshot([10, 0, 0, 1]))).unwrap();
        store_network_snapshot(&slot, Ok(ipv4_snapshot([10, 0, 0, 2]))).unwrap();

        let snapshot = take_network_snapshot(&slot).unwrap().unwrap();

        assert_eq!(snapshot.addresses[0].address, Ipv4Addr::new(10, 0, 0, 2));
        assert!(take_network_snapshot(&slot).unwrap().is_none());
    }

    #[test]
    fn unexpected_tick_error_resets_daemon_so_same_ip_is_redrawn_after_reconnect() {
        let start = Instant::now();
        let snapshot = ipv4_snapshot([192, 168, 1, 20]);
        let io = FakeIo {
            now: Cell::new(Some(start)),
            devices: vec![target_device()],
            snapshot_results: VecDeque::from([
                Ok(Some(snapshot.clone())),
                Ok(Some(snapshot.clone())),
                Ok(Some(snapshot)),
            ]),
            send_results: VecDeque::from([Ok(()), Ok(()), Err(other_error()), Ok(()), Ok(())]),
            ..FakeIo::default()
        };
        let mut runtime = Runtime::new(options(), io);

        runtime.tick().unwrap();
        runtime.io.set_now(start + Duration::from_secs(1));
        let err = runtime.tick().unwrap_err();
        runtime.recover_after_unexpected_tick_error(err);
        runtime.io.set_now(start + Duration::from_secs(2));
        runtime.tick().unwrap();

        assert_eq!(
            runtime
                .io
                .events
                .iter()
                .filter(|event| event.as_str() == "writes:pending")
                .count(),
            2
        );
        assert_eq!(
            runtime
                .io
                .events
                .iter()
                .filter(|event| event.as_str() == "writes:ip")
                .count(),
            2
        );
        assert!(runtime.io.events.iter().any(|event| event == "disconnect"));
    }
}
