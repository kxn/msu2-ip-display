#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerialErrorKind {
    Disconnected,
    Timeout,
    Other,
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
        assert_eq!(classify_poll_revents(POLLHUP_BITS), Some(SerialErrorKind::Disconnected));
        assert_eq!(classify_poll_revents(POLLERR_BITS), Some(SerialErrorKind::Disconnected));
        assert_eq!(classify_poll_revents(POLLNVAL_BITS), Some(SerialErrorKind::Disconnected));
    }
}
