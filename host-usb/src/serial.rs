#[cfg(target_os = "linux")]
use std::os::fd::{AsRawFd, RawFd};
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerialErrorKind {
    Disconnected,
    Timeout,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeoutCounter {
    consecutive: u8,
    threshold: u8,
}

impl TimeoutCounter {
    pub fn new(threshold: u8) -> Self {
        Self {
            consecutive: 0,
            threshold,
        }
    }

    pub fn record_success(&mut self) {
        self.consecutive = 0;
    }

    pub fn record_timeout(&mut self) -> bool {
        self.consecutive += 1;
        self.consecutive >= self.threshold
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
const POLLHUP_BITS: i16 = libc::POLLHUP as i16;
#[cfg(any(target_os = "linux", target_os = "android"))]
const POLLERR_BITS: i16 = libc::POLLERR as i16;
#[cfg(any(target_os = "linux", target_os = "android"))]
const POLLNVAL_BITS: i16 = libc::POLLNVAL as i16;

#[cfg(not(any(target_os = "linux", target_os = "android")))]
const POLLHUP_BITS: i16 = 0x0010;
#[cfg(not(any(target_os = "linux", target_os = "android")))]
const POLLERR_BITS: i16 = 0x0008;
#[cfg(not(any(target_os = "linux", target_os = "android")))]
const POLLNVAL_BITS: i16 = 0x0020;

pub fn classify_errno(_errno: i32) -> SerialErrorKind {
    match _errno {
        libc::EIO | libc::ENODEV | libc::ENXIO => SerialErrorKind::Disconnected,
        libc::EAGAIN => SerialErrorKind::Timeout,
        _ => SerialErrorKind::Other,
    }
}

pub fn classify_io_error(error: &std::io::Error) -> SerialErrorKind {
    if let Some(errno) = error.raw_os_error() {
        return classify_errno(errno);
    }

    match error.kind() {
        std::io::ErrorKind::TimedOut => SerialErrorKind::Timeout,
        std::io::ErrorKind::BrokenPipe
        | std::io::ErrorKind::ConnectionAborted
        | std::io::ErrorKind::ConnectionReset
        | std::io::ErrorKind::UnexpectedEof
        | std::io::ErrorKind::NotConnected => SerialErrorKind::Disconnected,
        _ => SerialErrorKind::Other,
    }
}

pub fn classify_poll_revents(revents: i16) -> Option<SerialErrorKind> {
    let disconnected = POLLHUP_BITS | POLLERR_BITS | POLLNVAL_BITS;
    if revents & disconnected != 0 {
        Some(SerialErrorKind::Disconnected)
    } else {
        None
    }
}

#[cfg(target_os = "linux")]
pub struct SerialSession {
    fd: std::os::fd::OwnedFd,
}

#[cfg(target_os = "linux")]
impl SerialSession {
    pub fn open(path: &std::path::Path) -> std::io::Result<Self> {
        use std::ffi::CString;
        use std::os::fd::FromRawFd;
        use std::os::unix::ffi::OsStrExt;

        let path = CString::new(path.as_os_str().as_bytes()).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "path contains nul byte")
        })?;
        let fd = unsafe {
            libc::open(
                path.as_ptr(),
                libc::O_RDWR | libc::O_NOCTTY | libc::O_NONBLOCK,
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }

        let fd = unsafe { std::os::fd::OwnedFd::from_raw_fd(fd) };
        configure_921600_rtscts(fd.as_raw_fd())?;
        Ok(Self { fd })
    }

    pub fn write_all(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        let deadline = Instant::now() + Duration::from_millis(500);
        let mut offset = 0;
        while offset < bytes.len() {
            let rc = unsafe {
                libc::write(
                    self.fd.as_raw_fd(),
                    bytes[offset..].as_ptr().cast(),
                    bytes.len() - offset,
                )
            };
            if rc < 0 {
                let err = std::io::Error::last_os_error();
                match classify_io_error(&err) {
                    SerialErrorKind::Timeout => {
                        let remaining = write_deadline_remaining(deadline)?;
                        if !self.wait_for(libc::POLLOUT, remaining, "write")? {
                            return Err(serial_timeout_error("write"));
                        }
                    }
                    _ => return Err(err),
                }
                continue;
            }
            if rc == 0 {
                let remaining = write_deadline_remaining(deadline)?;
                if !self.wait_for(libc::POLLOUT, remaining, "write")? {
                    return Err(serial_timeout_error("write"));
                }
                continue;
            }
            offset += rc as usize;
        }
        Ok(())
    }

    pub fn handshake(&mut self) -> std::io::Result<()> {
        self.write_all(&crate::protocol::HANDSHAKE)?;
        let reply = self.read_reply(Duration::from_millis(500))?;
        if crate::protocol::contains_sequence(&reply, &crate::protocol::HANDSHAKE) {
            Ok(())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "handshake did not echo MSNCN",
            ))
        }
    }

    pub fn send_writes(&mut self, writes: &[crate::display::WireWrite]) -> std::io::Result<()> {
        for write in writes {
            self.write_all(&write.bytes)?;
            if write.wait_for_echo {
                let _reply = self.read_reply(Duration::from_millis(500))?;
            }
        }
        Ok(())
    }

    fn read_reply(&mut self, timeout: Duration) -> std::io::Result<Vec<u8>> {
        const IDLE_TIMEOUT: Duration = Duration::from_millis(40);

        let deadline = Instant::now() + timeout;
        let mut out = Vec::new();
        let mut buf = [0u8; 256];

        loop {
            let now = Instant::now();
            if now >= deadline {
                break;
            }

            let remaining = deadline.saturating_duration_since(now);
            let wait = if out.is_empty() {
                remaining
            } else {
                remaining.min(IDLE_TIMEOUT)
            };

            if !self.wait_for(libc::POLLIN, wait, "read")? {
                break;
            }

            loop {
                let rc =
                    unsafe { libc::read(self.fd.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len()) };
                if rc > 0 {
                    out.extend_from_slice(&buf[..rc as usize]);
                    continue;
                }
                if rc == 0 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "serial device closed while reading",
                    ));
                }

                let err = std::io::Error::last_os_error();
                match classify_io_error(&err) {
                    SerialErrorKind::Timeout => break,
                    _ => return Err(err),
                }
            }
        }

        if out.is_empty() {
            Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "timed out waiting for serial reply",
            ))
        } else {
            Ok(out)
        }
    }

    fn wait_for(&self, events: i16, timeout: Duration, action: &str) -> std::io::Result<bool> {
        let mut pollfd = libc::pollfd {
            fd: self.fd.as_raw_fd(),
            events,
            revents: 0,
        };
        let timeout_ms = timeout
            .as_millis()
            .min(i32::MAX as u128)
            .try_into()
            .unwrap_or(i32::MAX);
        let rc = unsafe { libc::poll(&mut pollfd, 1, timeout_ms) };
        if rc < 0 {
            return Err(std::io::Error::last_os_error());
        }
        if let Some(kind) = classify_poll_revents(pollfd.revents) {
            return match kind {
                SerialErrorKind::Disconnected => Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    format!("serial device disconnected during {action}"),
                )),
                SerialErrorKind::Timeout | SerialErrorKind::Other => Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("serial poll failed during {action}"),
                )),
            };
        }
        if rc == 0 {
            return Ok(false);
        }
        Ok(true)
    }
}

#[cfg(target_os = "linux")]
fn write_deadline_remaining(deadline: Instant) -> std::io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|duration| !duration.is_zero())
        .ok_or_else(|| serial_timeout_error("write"))
}

#[cfg(target_os = "linux")]
fn serial_timeout_error(action: &str) -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::TimedOut,
        format!("timed out waiting for serial device during {action}"),
    )
}

#[cfg(target_os = "linux")]
fn configure_921600_rtscts(fd: RawFd) -> std::io::Result<()> {
    let mut termios = std::mem::MaybeUninit::<libc::termios>::uninit();
    if unsafe { libc::tcgetattr(fd, termios.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error());
    }

    let mut termios = unsafe { termios.assume_init() };
    unsafe { libc::cfmakeraw(&mut termios) };
    termios.c_cflag |= libc::CLOCAL | libc::CREAD | libc::CRTSCTS;
    termios.c_cflag &= !libc::CSIZE;
    termios.c_cflag |= libc::CS8;
    termios.c_cflag &= !libc::PARENB;
    termios.c_cflag &= !libc::CSTOPB;
    if unsafe { libc::cfsetspeed(&mut termios, libc::B921600) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &termios) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod timeout_counter_tests {
    use super::*;

    #[test]
    fn disconnects_after_three_consecutive_timeouts() {
        let mut counter = TimeoutCounter::new(3);
        assert!(!counter.record_timeout());
        assert!(!counter.record_timeout());
        assert!(counter.record_timeout());
        counter.record_success();
        assert!(!counter.record_timeout());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_gone_errors_are_disconnected() {
        assert_eq!(classify_errno(libc::EIO), SerialErrorKind::Disconnected);
        assert_eq!(classify_errno(libc::ENODEV), SerialErrorKind::Disconnected);
        assert_eq!(classify_errno(libc::ENXIO), SerialErrorKind::Disconnected);
    }

    #[test]
    fn poll_hangup_and_error_are_disconnected() {
        assert_eq!(
            classify_poll_revents(POLLHUP_BITS),
            Some(SerialErrorKind::Disconnected)
        );
        assert_eq!(
            classify_poll_revents(POLLERR_BITS),
            Some(SerialErrorKind::Disconnected)
        );
        assert_eq!(
            classify_poll_revents(POLLNVAL_BITS),
            Some(SerialErrorKind::Disconnected)
        );
    }

    #[test]
    fn timed_out_kind_maps_to_timeout() {
        let err = std::io::Error::new(std::io::ErrorKind::TimedOut, "later");
        assert_eq!(classify_io_error(&err), SerialErrorKind::Timeout);
    }
}
