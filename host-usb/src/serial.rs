#[cfg(target_os = "linux")]
use std::os::fd::{AsRawFd, RawFd};

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
                return Err(std::io::Error::last_os_error());
            }
            offset += rc as usize;
        }
        Ok(())
    }
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
}
