#[cfg(target_os = "linux")]
use std::ffi::CStr;
use std::net::Ipv4Addr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressCandidate {
    pub interface: String,
    pub address: Ipv4Addr,
    pub is_dynamic: bool,
    pub is_up: bool,
    pub is_lower_up: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    pub interface: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NetworkSnapshot {
    pub addresses: Vec<AddressCandidate>,
    pub routes: Vec<Route>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SelectionConfig {
    pub interface: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    Show(Ipv4Addr),
    Pending,
    FailureCandidate,
}

#[cfg(any(target_os = "linux", test))]
const IFA_F_PERMANENT_BITS: u32 = 0x80;
#[cfg(any(target_os = "linux", test))]
const INFINITE_LIFETIME: u32 = u32::MAX;
#[cfg(any(target_os = "linux", test))]
const AF_INET_BITS: u8 = 2;
#[cfg(any(target_os = "linux", test))]
const RTN_UNSPEC_BITS: u8 = 0;
#[cfg(any(target_os = "linux", test))]
const RTN_UNICAST_BITS: u8 = 1;

#[cfg(target_os = "linux")]
pub fn collect_network_snapshot() -> std::io::Result<NetworkSnapshot> {
    let mut snapshot = NetworkSnapshot::default();
    collect_ipv4_addresses_getifaddrs(&mut snapshot)?;
    if let Err(err) = enrich_dynamic_flags_from_rtnetlink(&mut snapshot) {
        crate::logging::debug(&format!("rtnetlink dynamic enrichment unavailable: {err}"));
    }
    collect_default_routes_rtnetlink(&mut snapshot)?;
    Ok(snapshot)
}

#[cfg(not(target_os = "linux"))]
pub fn collect_network_snapshot() -> std::io::Result<NetworkSnapshot> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "network snapshot collection is only supported on Linux",
    ))
}

pub fn select_ipv4(snapshot: &NetworkSnapshot, config: &SelectionConfig) -> Selection {
    let candidates: Vec<&AddressCandidate> = snapshot
        .addresses
        .iter()
        .filter(|candidate| candidate.is_up && candidate.is_lower_up)
        .collect();

    if let Some(interface) = &config.interface {
        return candidates
            .iter()
            .copied()
            .find(|candidate| {
                candidate.interface == *interface && is_normal_ipv4(candidate.address)
            })
            .map(|candidate| Selection::Show(candidate.address))
            .unwrap_or_else(|| {
                if candidates.iter().any(|candidate| {
                    candidate.interface == *interface && is_link_local(candidate.address)
                }) {
                    Selection::FailureCandidate
                } else {
                    Selection::Pending
                }
            });
    }

    if let Some(default_route) = snapshot.routes.iter().find(|route| route.is_default) {
        if let Some(candidate) = candidates.iter().copied().find(|candidate| {
            candidate.interface == default_route.interface && is_normal_ipv4(candidate.address)
        }) {
            return Selection::Show(candidate.address);
        }
    }

    let normal: Vec<&AddressCandidate> = candidates
        .iter()
        .copied()
        .filter(|candidate| is_normal_ipv4(candidate.address))
        .filter(|candidate| !is_virtual_interface(&candidate.interface))
        .collect();

    match normal.len() {
        0 => {
            if candidates.iter().any(|candidate| {
                !is_virtual_interface(&candidate.interface) && is_link_local(candidate.address)
            }) {
                Selection::FailureCandidate
            } else {
                Selection::Pending
            }
        }
        1 => Selection::Show(normal[0].address),
        _ => {
            let dynamic: Vec<&AddressCandidate> = normal
                .iter()
                .copied()
                .filter(|candidate| candidate.is_dynamic)
                .collect();
            if dynamic.len() == 1 {
                Selection::Show(dynamic[0].address)
            } else {
                Selection::FailureCandidate
            }
        }
    }
}

fn is_normal_ipv4(address: Ipv4Addr) -> bool {
    !address.is_unspecified()
        && !address.is_loopback()
        && !is_link_local(address)
        && !address.is_multicast()
}

fn is_link_local(address: Ipv4Addr) -> bool {
    let octets = address.octets();
    octets[0] == 169 && octets[1] == 254
}

fn is_virtual_interface(name: &str) -> bool {
    name == "lo"
        || name.starts_with("docker")
        || name.starts_with("br-")
        || name.starts_with("veth")
        || name.starts_with("virbr")
        || name.starts_with("tun")
        || name.starts_with("tap")
        || name.starts_with("wg")
}

#[cfg(any(target_os = "linux", test))]
fn address_is_dynamic(
    flags: u32,
    preferred_lifetime: Option<u32>,
    valid_lifetime: Option<u32>,
) -> bool {
    (flags & IFA_F_PERMANENT_BITS) == 0
        || preferred_lifetime.is_some_and(|lifetime| lifetime != INFINITE_LIFETIME)
        || valid_lifetime.is_some_and(|lifetime| lifetime != INFINITE_LIFETIME)
}

#[cfg(any(target_os = "linux", test))]
fn route_is_default_ipv4(family: u8, dst_len: u8, route_type: u8) -> bool {
    family == AF_INET_BITS
        && dst_len == 0
        && matches!(route_type, RTN_UNSPEC_BITS | RTN_UNICAST_BITS)
}

#[cfg(target_os = "linux")]
fn collect_ipv4_addresses_getifaddrs(snapshot: &mut NetworkSnapshot) -> std::io::Result<()> {
    let mut addrs = std::ptr::null_mut();
    if unsafe { libc::getifaddrs(&mut addrs) } != 0 {
        return Err(std::io::Error::last_os_error());
    }

    let mut current = addrs;
    while !current.is_null() {
        let ifaddr = unsafe { &*current };
        let sockaddr = ifaddr.ifa_addr;
        if !sockaddr.is_null() && unsafe { (*sockaddr).sa_family as i32 } == libc::AF_INET {
            if let Some(address) = sockaddr_to_ipv4(sockaddr) {
                let interface = unsafe { CStr::from_ptr(ifaddr.ifa_name) }
                    .to_string_lossy()
                    .into_owned();
                let flags = ifaddr.ifa_flags;
                let is_up = flags & libc::IFF_UP as u32 != 0;
                let is_lower_up = flags & libc::IFF_RUNNING as u32 != 0
                    || (is_up && flags & libc::IFF_LOOPBACK as u32 == 0);

                if !snapshot.addresses.iter().any(|candidate| {
                    candidate.interface == interface && candidate.address == address
                }) {
                    snapshot.addresses.push(AddressCandidate {
                        interface,
                        address,
                        is_dynamic: false,
                        is_up,
                        is_lower_up,
                    });
                }
            }
        }
        current = ifaddr.ifa_next;
    }

    unsafe {
        libc::freeifaddrs(addrs);
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn enrich_dynamic_flags_from_rtnetlink(snapshot: &mut NetworkSnapshot) -> std::io::Result<()> {
    let request = NetlinkRequest {
        header: libc::nlmsghdr {
            nlmsg_len: std::mem::size_of::<NetlinkRequest<IfAddrMsg>>() as u32,
            nlmsg_type: libc::RTM_GETADDR,
            nlmsg_flags: (libc::NLM_F_REQUEST | libc::NLM_F_DUMP) as u16,
            nlmsg_seq: 1,
            nlmsg_pid: 0,
        },
        body: IfAddrMsg {
            ifa_family: AF_INET_BITS,
            ifa_prefixlen: 0,
            ifa_flags: 0,
            ifa_scope: 0,
            ifa_index: 0,
        },
    };

    let response = netlink_dump(&request, libc::NETLINK_ROUTE)?;
    parse_netlink_messages(&response, |message_type, payload| {
        if message_type != libc::RTM_NEWADDR {
            return Ok(());
        }

        if payload.len() < std::mem::size_of::<IfAddrMsg>() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "short ifaddrmsg payload",
            ));
        }
        let message = unsafe { &*(payload.as_ptr().cast::<IfAddrMsg>()) };
        if message.ifa_family != AF_INET_BITS {
            return Ok(());
        }

        let mut address = None;
        let mut attr_flags = None;
        let mut preferred_lifetime = None;
        let mut valid_lifetime = None;

        parse_rtattrs(
            &payload[std::mem::size_of::<IfAddrMsg>()..],
            |kind, data| {
                match kind {
                    value if value == libc::IFA_LOCAL || value == libc::IFA_ADDRESS => {
                        if address.is_none() {
                            address = bytes_to_ipv4(data);
                        }
                    }
                    value if value == libc::IFA_FLAGS => {
                        attr_flags = read_u32(data);
                    }
                    value if value == libc::IFA_CACHEINFO => {
                        if data.len() >= std::mem::size_of::<IfaCacheInfo>() {
                            let cache = unsafe { &*(data.as_ptr().cast::<IfaCacheInfo>()) };
                            preferred_lifetime = Some(cache.ifa_prefered);
                            valid_lifetime = Some(cache.ifa_valid);
                        }
                    }
                    _ => {}
                }
                Ok(())
            },
        )?;

        let Some(address) = address else {
            return Ok(());
        };
        let Some(interface) = if_indextoname_string(message.ifa_index)? else {
            return Ok(());
        };
        let flags = attr_flags.unwrap_or(message.ifa_flags as u32);

        if let Some(candidate) = snapshot
            .addresses
            .iter_mut()
            .find(|candidate| candidate.interface == interface && candidate.address == address)
        {
            candidate.is_dynamic = address_is_dynamic(flags, preferred_lifetime, valid_lifetime);
        }

        Ok(())
    })
}

#[cfg(target_os = "linux")]
fn collect_default_routes_rtnetlink(snapshot: &mut NetworkSnapshot) -> std::io::Result<()> {
    let request = NetlinkRequest {
        header: libc::nlmsghdr {
            nlmsg_len: std::mem::size_of::<NetlinkRequest<RtMsg>>() as u32,
            nlmsg_type: libc::RTM_GETROUTE,
            nlmsg_flags: (libc::NLM_F_REQUEST | libc::NLM_F_DUMP) as u16,
            nlmsg_seq: 2,
            nlmsg_pid: 0,
        },
        body: RtMsg {
            rtm_family: AF_INET_BITS,
            rtm_dst_len: 0,
            rtm_src_len: 0,
            rtm_tos: 0,
            rtm_table: 0,
            rtm_protocol: 0,
            rtm_scope: 0,
            rtm_type: 0,
            rtm_flags: 0,
        },
    };

    let response = netlink_dump(&request, libc::NETLINK_ROUTE)?;
    parse_netlink_messages(&response, |message_type, payload| {
        if message_type != libc::RTM_NEWROUTE {
            return Ok(());
        }
        if payload.len() < std::mem::size_of::<RtMsg>() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "short rtmsg payload",
            ));
        }
        let message = unsafe { &*(payload.as_ptr().cast::<RtMsg>()) };
        if !route_is_default_ipv4(message.rtm_family, message.rtm_dst_len, message.rtm_type) {
            return Ok(());
        }

        let mut interface_index = None;
        parse_rtattrs(&payload[std::mem::size_of::<RtMsg>()..], |kind, data| {
            if kind == libc::RTA_OIF {
                interface_index = read_u32(data);
            }
            Ok(())
        })?;

        let Some(interface_index) = interface_index else {
            return Ok(());
        };
        let Some(interface) = if_indextoname_string(interface_index)? else {
            return Ok(());
        };
        if !snapshot
            .routes
            .iter()
            .any(|route| route.interface == interface && route.is_default)
        {
            snapshot.routes.push(Route {
                interface,
                is_default: true,
            });
        }
        Ok(())
    })
}

#[cfg(target_os = "linux")]
#[repr(C)]
struct NetlinkRequest<T> {
    header: libc::nlmsghdr,
    body: T,
}

#[cfg(target_os = "linux")]
#[repr(C)]
struct IfAddrMsg {
    ifa_family: u8,
    ifa_prefixlen: u8,
    ifa_flags: u8,
    ifa_scope: u8,
    ifa_index: u32,
}

#[cfg(target_os = "linux")]
#[repr(C)]
struct IfaCacheInfo {
    ifa_prefered: u32,
    ifa_valid: u32,
    cstamp: u32,
    tstamp: u32,
}

#[cfg(target_os = "linux")]
#[repr(C)]
struct RtMsg {
    rtm_family: u8,
    rtm_dst_len: u8,
    rtm_src_len: u8,
    rtm_tos: u8,
    rtm_table: u8,
    rtm_protocol: u8,
    rtm_scope: u8,
    rtm_type: u8,
    rtm_flags: u32,
}

#[cfg(target_os = "linux")]
#[repr(C)]
struct RtAttr {
    rta_len: u16,
    rta_type: u16,
}

#[cfg(target_os = "linux")]
fn netlink_dump<T>(request: &NetlinkRequest<T>, protocol: i32) -> std::io::Result<Vec<u8>> {
    let fd = unsafe { libc::socket(libc::AF_NETLINK, libc::SOCK_RAW, protocol) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }

    let local = netlink_sockaddr(0);
    if unsafe {
        libc::bind(
            fd,
            (&local as *const libc::sockaddr_nl).cast(),
            std::mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t,
        )
    } != 0
    {
        let err = std::io::Error::last_os_error();
        unsafe {
            libc::close(fd);
        }
        return Err(err);
    }

    let kernel = netlink_sockaddr(0);
    let send_rc = unsafe {
        libc::sendto(
            fd,
            (request as *const NetlinkRequest<T>).cast(),
            request.header.nlmsg_len as usize,
            0,
            (&kernel as *const libc::sockaddr_nl).cast(),
            std::mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t,
        )
    };
    if send_rc < 0 {
        let err = std::io::Error::last_os_error();
        unsafe {
            libc::close(fd);
        }
        return Err(err);
    }

    let mut out = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        let count = unsafe { libc::recv(fd, buf.as_mut_ptr().cast(), buf.len(), 0) };
        if count < 0 {
            let err = std::io::Error::last_os_error();
            unsafe {
                libc::close(fd);
            }
            return Err(err);
        }
        if count == 0 {
            break;
        }
        out.extend_from_slice(&buf[..count as usize]);
        if netlink_buffer_is_done(&buf[..count as usize])? {
            break;
        }
    }

    unsafe {
        libc::close(fd);
    }
    Ok(out)
}

#[cfg(target_os = "linux")]
fn netlink_buffer_is_done(buffer: &[u8]) -> std::io::Result<bool> {
    let mut offset = 0usize;
    while offset + std::mem::size_of::<libc::nlmsghdr>() <= buffer.len() {
        let header = unsafe { &*(buffer[offset..].as_ptr().cast::<libc::nlmsghdr>()) };
        if header.nlmsg_len < std::mem::size_of::<libc::nlmsghdr>() as u32 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid netlink header length",
            ));
        }
        if header.nlmsg_type == libc::NLMSG_DONE as u16 {
            return Ok(true);
        }
        offset += nlmsg_align(header.nlmsg_len as usize);
    }
    Ok(false)
}

#[cfg(target_os = "linux")]
fn parse_netlink_messages<F>(buffer: &[u8], mut visit: F) -> std::io::Result<()>
where
    F: FnMut(u16, &[u8]) -> std::io::Result<()>,
{
    let mut offset = 0usize;
    while offset + std::mem::size_of::<libc::nlmsghdr>() <= buffer.len() {
        let header = unsafe { &*(buffer[offset..].as_ptr().cast::<libc::nlmsghdr>()) };
        if header.nlmsg_len < std::mem::size_of::<libc::nlmsghdr>() as u32 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid netlink message length",
            ));
        }
        let end = offset + header.nlmsg_len as usize;
        if end > buffer.len() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "truncated netlink message",
            ));
        }
        let payload = &buffer[offset + std::mem::size_of::<libc::nlmsghdr>()..end];
        match header.nlmsg_type {
            value if value == libc::NLMSG_DONE as u16 => break,
            value if value == libc::NLMSG_ERROR as u16 => {
                if payload.len() < std::mem::size_of::<libc::nlmsgerr>() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "short netlink error payload",
                    ));
                }
                let message = unsafe { &*(payload.as_ptr().cast::<libc::nlmsgerr>()) };
                if message.error == 0 {
                    break;
                }
                return Err(std::io::Error::from_raw_os_error(-message.error));
            }
            message_type => visit(message_type, payload)?,
        }
        offset += nlmsg_align(header.nlmsg_len as usize);
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn parse_rtattrs<F>(payload: &[u8], mut visit: F) -> std::io::Result<()>
where
    F: FnMut(u16, &[u8]) -> std::io::Result<()>,
{
    let mut offset = 0usize;
    while offset + std::mem::size_of::<RtAttr>() <= payload.len() {
        let attr = unsafe { &*(payload[offset..].as_ptr().cast::<RtAttr>()) };
        let attr_len = attr.rta_len as usize;
        if attr_len < std::mem::size_of::<RtAttr>() || offset + attr_len > payload.len() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid route attribute length",
            ));
        }
        let data_start = offset + std::mem::size_of::<RtAttr>();
        let data = &payload[data_start..offset + attr_len];
        visit(attr.rta_type, data)?;
        offset += rta_align(attr_len);
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn netlink_sockaddr(groups: u32) -> libc::sockaddr_nl {
    let mut address: libc::sockaddr_nl = unsafe { std::mem::zeroed() };
    address.nl_family = libc::AF_NETLINK as libc::sa_family_t;
    address.nl_pid = 0;
    address.nl_groups = groups;
    address
}

#[cfg(target_os = "linux")]
fn if_indextoname_string(index: u32) -> std::io::Result<Option<String>> {
    let mut name = [0i8; libc::IF_NAMESIZE];
    let ptr = unsafe { libc::if_indextoname(index, name.as_mut_ptr()) };
    if ptr.is_null() {
        let err = std::io::Error::last_os_error();
        return if err.raw_os_error() == Some(libc::ENXIO) {
            Ok(None)
        } else {
            Err(err)
        };
    }
    Ok(Some(
        unsafe { CStr::from_ptr(name.as_ptr()) }
            .to_string_lossy()
            .into_owned(),
    ))
}

#[cfg(target_os = "linux")]
fn sockaddr_to_ipv4(sockaddr: *const libc::sockaddr) -> Option<Ipv4Addr> {
    let address = unsafe { &*(sockaddr.cast::<libc::sockaddr_in>()) }
        .sin_addr
        .s_addr;
    Some(Ipv4Addr::from(u32::from_be(address)))
}

#[cfg(target_os = "linux")]
fn bytes_to_ipv4(data: &[u8]) -> Option<Ipv4Addr> {
    if data.len() < 4 {
        return None;
    }
    Some(Ipv4Addr::new(data[0], data[1], data[2], data[3]))
}

#[cfg(target_os = "linux")]
fn read_u32(data: &[u8]) -> Option<u32> {
    if data.len() < std::mem::size_of::<u32>() {
        return None;
    }
    Some(u32::from_ne_bytes([data[0], data[1], data[2], data[3]]))
}

#[cfg(target_os = "linux")]
fn nlmsg_align(len: usize) -> usize {
    (len + 3) & !3
}

#[cfg(target_os = "linux")]
fn rta_align(len: usize) -> usize {
    (len + 3) & !3
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(interface: &str, address: [u8; 4], is_dynamic: bool) -> AddressCandidate {
        AddressCandidate {
            interface: interface.to_string(),
            address: Ipv4Addr::from(address),
            is_dynamic,
            is_up: true,
            is_lower_up: true,
        }
    }

    #[test]
    fn fixed_interface_wins_over_default_route() {
        let snapshot = NetworkSnapshot {
            addresses: vec![
                addr("eth0", [192, 168, 1, 10], true),
                addr("eth1", [10, 0, 0, 5], true),
            ],
            routes: vec![Route {
                interface: "eth1".to_string(),
                is_default: true,
            }],
        };
        let config = SelectionConfig {
            interface: Some("eth0".to_string()),
        };

        assert_eq!(
            select_ipv4(&snapshot, &config),
            Selection::Show(Ipv4Addr::new(192, 168, 1, 10))
        );
    }

    #[test]
    fn default_route_interface_is_preferred() {
        let snapshot = NetworkSnapshot {
            addresses: vec![
                addr("eth0", [192, 168, 1, 10], true),
                addr("eth1", [10, 0, 0, 5], true),
            ],
            routes: vec![Route {
                interface: "eth1".to_string(),
                is_default: true,
            }],
        };

        assert_eq!(
            select_ipv4(&snapshot, &SelectionConfig::default()),
            Selection::Show(Ipv4Addr::new(10, 0, 0, 5))
        );
    }

    #[test]
    fn isolated_single_normal_ipv4_is_displayed() {
        let snapshot = NetworkSnapshot {
            addresses: vec![addr("eth0", [192, 168, 55, 20], true)],
            routes: vec![],
        };

        assert_eq!(
            select_ipv4(&snapshot, &SelectionConfig::default()),
            Selection::Show(Ipv4Addr::new(192, 168, 55, 20))
        );
    }

    #[test]
    fn multiple_without_default_prefers_dynamic() {
        let snapshot = NetworkSnapshot {
            addresses: vec![
                addr("eth0", [192, 168, 1, 10], false),
                addr("eth1", [10, 0, 0, 5], true),
            ],
            routes: vec![],
        };

        assert_eq!(
            select_ipv4(&snapshot, &SelectionConfig::default()),
            Selection::Show(Ipv4Addr::new(10, 0, 0, 5))
        );
    }

    #[test]
    fn link_local_only_is_failure_candidate_after_delay() {
        let snapshot = NetworkSnapshot {
            addresses: vec![addr("eth0", [169, 254, 1, 2], true)],
            routes: vec![],
        };

        assert_eq!(
            select_ipv4(&snapshot, &SelectionConfig::default()),
            Selection::FailureCandidate
        );
    }

    #[test]
    fn virtual_interfaces_are_ignored_for_fallback() {
        let snapshot = NetworkSnapshot {
            addresses: vec![addr("docker0", [172, 17, 0, 1], false)],
            routes: vec![],
        };

        assert_eq!(
            select_ipv4(&snapshot, &SelectionConfig::default()),
            Selection::Pending
        );
    }

    #[test]
    fn virtual_link_local_only_is_pending_for_fallback() {
        let snapshot = NetworkSnapshot {
            addresses: vec![addr("docker0", [169, 254, 1, 2], true)],
            routes: vec![],
        };

        assert_eq!(
            select_ipv4(&snapshot, &SelectionConfig::default()),
            Selection::Pending
        );
    }

    #[test]
    fn permanent_flags_without_finite_lifetimes_are_not_dynamic() {
        assert!(!address_is_dynamic(
            IFA_F_PERMANENT_BITS,
            Some(INFINITE_LIFETIME),
            Some(INFINITE_LIFETIME)
        ));
    }

    #[test]
    fn finite_lifetime_or_non_permanent_flags_mark_address_dynamic() {
        assert!(address_is_dynamic(0, None, None));
        assert!(address_is_dynamic(
            IFA_F_PERMANENT_BITS,
            Some(30),
            Some(INFINITE_LIFETIME)
        ));
    }

    #[test]
    fn default_route_helper_accepts_only_ipv4_default_unicast() {
        assert!(route_is_default_ipv4(AF_INET_BITS, 0, RTN_UNICAST_BITS));
        assert!(!route_is_default_ipv4(AF_INET_BITS, 24, RTN_UNICAST_BITS));
    }
}
