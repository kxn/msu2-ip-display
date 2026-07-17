#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventMode {
    Netlink,
    Polling,
}

#[cfg(target_os = "linux")]
pub fn choose_event_mode() -> EventMode {
    if netlink_socket_available() {
        EventMode::Netlink
    } else {
        EventMode::Polling
    }
}

#[cfg(target_os = "linux")]
fn netlink_socket_available() -> bool {
    let fd = unsafe {
        libc::socket(
            libc::AF_NETLINK,
            libc::SOCK_DGRAM,
            libc::NETLINK_KOBJECT_UEVENT,
        )
    };
    if fd < 0 {
        return false;
    }
    unsafe { libc::close(fd) };
    true
}

#[cfg(not(target_os = "linux"))]
pub fn choose_event_mode() -> EventMode {
    EventMode::Polling
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_mode_values_are_stable_for_logging() {
        assert_eq!(format!("{:?}", EventMode::Netlink), "Netlink");
        assert_eq!(format!("{:?}", EventMode::Polling), "Polling");
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn non_linux_uses_polling_mode() {
        assert_eq!(choose_event_mode(), EventMode::Polling);
    }
}
