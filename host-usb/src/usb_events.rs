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
    netlink_socket_available_with(
        || unsafe {
            libc::socket(
                libc::AF_NETLINK,
                libc::SOCK_DGRAM,
                libc::NETLINK_KOBJECT_UEVENT,
            )
        },
        bind_uevent_netlink,
        |fd| unsafe {
            libc::close(fd);
        },
    )
}

#[cfg(target_os = "linux")]
fn bind_uevent_netlink(fd: libc::c_int) -> bool {
    let addr = libc::sockaddr_nl {
        nl_family: libc::AF_NETLINK as libc::sa_family_t,
        nl_pad: 0,
        nl_pid: 0,
        nl_groups: 1,
    };
    let rc = unsafe {
        libc::bind(
            fd,
            (&addr as *const libc::sockaddr_nl).cast(),
            std::mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t,
        )
    };
    rc == 0
}

#[cfg(any(target_os = "linux", test))]
fn netlink_socket_available_with<S, B, C>(socket: S, bind: B, close: C) -> bool
where
    S: FnOnce() -> libc::c_int,
    B: FnOnce(libc::c_int) -> bool,
    C: FnOnce(libc::c_int),
{
    let fd = socket();
    if fd < 0 {
        return false;
    }

    let bound = bind(fd);
    close(fd);
    bound
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

    #[test]
    fn netlink_probe_reports_failure_when_bind_fails() {
        let mut closed = false;
        let available = netlink_socket_available_with(|| 7, |_| false, |_| closed = true);
        assert!(!available);
        assert!(closed);
    }

    #[test]
    fn netlink_probe_reports_success_when_bind_succeeds() {
        let mut closed = false;
        let available = netlink_socket_available_with(|| 7, |_| true, |_| closed = true);
        assert!(available);
        assert!(closed);
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn non_linux_uses_polling_mode() {
        assert_eq!(choose_event_mode(), EventMode::Polling);
    }
}
