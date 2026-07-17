use std::io;
use std::time::{Duration, Instant};

use crate::cli::RunOptions;
use crate::daemon::{Daemon, DaemonAction, DaemonEvent, DaemonState};
use crate::device_scan::TtyDevice;
use crate::display::{DisplayRenderer, WireWrite};
use crate::ip_detect::NetworkSnapshot;
use crate::serial::{classify_io_error, SerialErrorKind, TimeoutCounter};

pub trait RuntimeIo {
    fn scan_devices(&mut self) -> io::Result<Vec<TtyDevice>>;
    fn connect(&mut self, device: &TtyDevice) -> io::Result<()>;
    fn disconnect(&mut self);
    fn network_snapshot(&mut self) -> io::Result<NetworkSnapshot>;
    fn send_writes(&mut self, writes: &[WireWrite]) -> io::Result<()>;
    fn sleep(&mut self, duration: Duration);
    fn now(&self) -> Instant;
}

pub struct Runtime<T> {
    daemon: Daemon,
    io: T,
    connected: bool,
    last_keepalive: Instant,
    timeout_counter: TimeoutCounter,
    pending_display_action: Option<DaemonAction>,
}

impl<T: RuntimeIo> Runtime<T> {
    pub fn new(options: RunOptions, io: T) -> Self {
        let now = io.now();
        Self {
            daemon: Daemon::new(options),
            io,
            connected: false,
            last_keepalive: now,
            timeout_counter: TimeoutCounter::new(3),
            pending_display_action: None,
        }
    }

    pub fn tick(&mut self) -> io::Result<()> {
        if !self.connected {
            let devices = self.io.scan_devices()?;
            let Some(device) = devices.first() else {
                if self.daemon.state != DaemonState::Listening {
                    self.disconnect_device();
                }
                self.io.sleep(Duration::from_millis(500));
                return Ok(());
            };

            if self.daemon.state == DaemonState::Listening {
                for action in self.daemon.handle_event(DaemonEvent::DeviceCandidateFound) {
                    self.apply_action(action)?;
                }
            }

            if let Err(err) = self.io.connect(device) {
                return self.handle_runtime_error("connect", err, true);
            }
            self.connected = true;
            self.timeout_counter.record_success();

            for action in self.daemon.handle_event(DaemonEvent::HandshakeOk) {
                if let Err(err) = self.apply_daemon_action(action) {
                    return self.handle_runtime_error("show pending", err, true);
                }
            }
        }

        if let Some(action) = self.pending_display_action.clone() {
            if let Err(err) = self.apply_action(action) {
                return self.handle_runtime_error("display retry", err, true);
            }
            self.pending_display_action = None;
        }

        let snapshot = match self.io.network_snapshot() {
            Ok(snapshot) => snapshot,
            Err(err) => return self.handle_runtime_error("network snapshot", err, false),
        };
        let now = self.io.now();

        for action in self
            .daemon
            .handle_event(DaemonEvent::NetworkSnapshot { snapshot, now })
        {
            if let Err(err) = self.apply_daemon_action(action) {
                return self.handle_runtime_error("display update", err, true);
            }
        }

        if now.duration_since(self.last_keepalive) >= Duration::from_secs(10) {
            if let Err(err) = self.io.send_writes(&DisplayRenderer::keepalive()) {
                return self.handle_runtime_error("keepalive", err, true);
            }
            self.timeout_counter.record_success();
            self.last_keepalive = now;
        }

        self.io.sleep(Duration::from_millis(500));
        Ok(())
    }

    fn apply_daemon_action(&mut self, action: DaemonAction) -> io::Result<()> {
        let is_display_action = matches!(
            action,
            DaemonAction::ShowPending | DaemonAction::ShowDhcpFailed | DaemonAction::ShowIp(_)
        );
        if is_display_action {
            self.pending_display_action = Some(action.clone());
        }
        self.apply_action(action)?;
        if is_display_action {
            self.pending_display_action = None;
        }
        Ok(())
    }

    fn apply_action(&mut self, action: DaemonAction) -> io::Result<()> {
        match action {
            DaemonAction::OpenDevice => Ok(()),
            DaemonAction::CloseDevice => {
                self.io.disconnect();
                self.connected = false;
                Ok(())
            }
            DaemonAction::ShowPending => {
                self.io.send_writes(&DisplayRenderer::pending())?;
                self.timeout_counter.record_success();
                Ok(())
            }
            DaemonAction::ShowDhcpFailed => {
                self.io.send_writes(&DisplayRenderer::dhcp_failed())?;
                self.timeout_counter.record_success();
                Ok(())
            }
            DaemonAction::ShowIp(ip) => {
                self.io.send_writes(&DisplayRenderer::ip(ip))?;
                self.timeout_counter.record_success();
                Ok(())
            }
        }
    }

    fn disconnect_device(&mut self) {
        self.pending_display_action = None;
        for action in self.daemon.handle_event(DaemonEvent::DeviceDisconnected) {
            let _ = self.apply_action(action);
        }
        self.timeout_counter.record_success();
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
}

#[cfg(target_os = "linux")]
pub struct LinuxRuntimeIo {
    session: Option<crate::serial::SerialSession>,
}

#[cfg(target_os = "linux")]
impl LinuxRuntimeIo {
    pub fn new() -> Self {
        Self { session: None }
    }
}

#[cfg(target_os = "linux")]
impl RuntimeIo for LinuxRuntimeIo {
    fn scan_devices(&mut self) -> io::Result<Vec<TtyDevice>> {
        crate::device_scan::scan_target_ttys()
    }

    fn connect(&mut self, device: &TtyDevice) -> io::Result<()> {
        self.session = None;
        let mut session = crate::serial::SerialSession::open(&device.path)?;
        session.handshake()?;
        self.session = Some(session);
        Ok(())
    }

    fn disconnect(&mut self) {
        self.session = None;
    }

    fn network_snapshot(&mut self) -> io::Result<NetworkSnapshot> {
        crate::ip_detect::collect_network_snapshot()
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

#[cfg(target_os = "linux")]
pub fn run_forever(options: RunOptions) -> io::Result<()> {
    let mut runtime = Runtime::new(options, LinuxRuntimeIo::new());
    loop {
        if let Err(err) = runtime.tick() {
            crate::logging::warn(&format!("runtime tick failed: {err}"));
            runtime.connected = false;
            runtime.io.disconnect();
            runtime.io.sleep(Duration::from_millis(500));
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

    use crate::display::DisplayRenderer;
    use crate::ip_detect::{AddressCandidate, Route};
    use crate::protocol::{show_photo_packet, IP_BACKGROUND_PAGE};

    #[derive(Default)]
    struct FakeIo {
        now: Cell<Option<Instant>>,
        devices: Vec<TtyDevice>,
        connect_results: VecDeque<io::Result<()>>,
        snapshot_results: VecDeque<io::Result<NetworkSnapshot>>,
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

        fn network_snapshot(&mut self) -> io::Result<NetworkSnapshot> {
            self.events.push("snapshot".to_string());
            self.snapshot_results
                .pop_front()
                .unwrap_or_else(|| Ok(NetworkSnapshot::default()))
        }

        fn send_writes(&mut self, writes: &[WireWrite]) -> io::Result<()> {
            let marker = if writes == DisplayRenderer::pending() {
                "pending"
            } else if writes == DisplayRenderer::dhcp_failed() {
                "dhcp_failed"
            } else if writes
                .first()
                .is_some_and(|write| write.bytes == show_photo_packet(IP_BACKGROUND_PAGE))
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

    fn options() -> RunOptions {
        RunOptions {
            interface: None,
            dhcp_fail_delay: Duration::from_secs(45),
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

    #[test]
    fn tick_connects_device_and_renders_pending_then_ip() {
        let start = Instant::now();
        let io = FakeIo {
            now: Cell::new(Some(start)),
            devices: vec![target_device()],
            snapshot_results: VecDeque::from([Ok(ipv4_snapshot([192, 168, 1, 20]))]),
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
    fn tick_disconnects_after_three_consecutive_serial_timeouts() {
        let start = Instant::now();
        let io = FakeIo {
            now: Cell::new(Some(start)),
            devices: vec![target_device()],
            snapshot_results: VecDeque::from([
                Ok(ipv4_snapshot([192, 168, 1, 20])),
                Ok(ipv4_snapshot([192, 168, 1, 20])),
                Ok(ipv4_snapshot([192, 168, 1, 20])),
                Ok(ipv4_snapshot([192, 168, 1, 20])),
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
                Ok(ipv4_snapshot([192, 168, 1, 20])),
                Ok(ipv4_snapshot([192, 168, 1, 20])),
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
                Ok(ipv4_snapshot([192, 168, 1, 20])),
                Ok(ipv4_snapshot([192, 168, 1, 20])),
                Ok(ipv4_snapshot([192, 168, 1, 21])),
                Ok(ipv4_snapshot([192, 168, 1, 22])),
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
            snapshot_results: VecDeque::from([Ok(snapshot.clone()), Ok(snapshot)]),
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
}
