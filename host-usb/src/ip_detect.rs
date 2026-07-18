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
#[cfg(any(target_os = "linux", test))]
const IFF_UP_BITS: u32 = 0x1;
#[cfg(any(target_os = "linux", test))]
const IFF_RUNNING_BITS: u32 = 0x40;
#[cfg(any(target_os = "linux", test))]
const RT_TABLE_MAIN_BITS: u32 = 254;
#[cfg(any(target_os = "linux", test))]
const NLMSG_ERROR_BITS: u16 = 2;
#[cfg(any(target_os = "linux", test))]
const NLMSG_DONE_BITS: u16 = 3;
#[cfg(any(target_os = "linux", test))]
const RTA_TABLE_BITS: u16 = 15;
#[cfg(any(target_os = "linux", test))]
const NETLINK_RECV_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(200);

#[cfg(target_os = "linux")]
pub fn collect_network_snapshot() -> std::io::Result<NetworkSnapshot> {
    collect_network_snapshot_best_effort(
        collect_ipv4_addresses_getifaddrs,
        enrich_dynamic_flags_from_rtnetlink,
        collect_default_routes_rtnetlink,
    )
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

#[cfg(any(target_os = "linux", test))]
fn collect_network_snapshot_best_effort<C, D, R>(
    collect_addresses: C,
    enrich_dynamic: D,
    collect_routes: R,
) -> std::io::Result<NetworkSnapshot>
where
    C: FnOnce(&mut NetworkSnapshot) -> std::io::Result<()>,
    D: FnOnce(&mut NetworkSnapshot) -> std::io::Result<()>,
    R: FnOnce(&mut NetworkSnapshot) -> std::io::Result<()>,
{
    let mut snapshot = NetworkSnapshot::default();
    collect_addresses(&mut snapshot)?;

    if let Err(err) = enrich_dynamic(&mut snapshot) {
        crate::logging::debug(&format!("rtnetlink dynamic enrichment unavailable: {err}"));
    }
    if let Err(err) = collect_routes(&mut snapshot) {
        crate::logging::debug(&format!("rtnetlink route enrichment unavailable: {err}"));
    }

    Ok(snapshot)
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

#[cfg(any(target_os = "linux", test))]
fn interface_is_lower_up(flags: u32) -> bool {
    flags & IFF_RUNNING_BITS != 0
}

#[cfg(any(target_os = "linux", test))]
fn route_is_main_default_ipv4(
    family: u8,
    dst_len: u8,
    route_type: u8,
    rtm_table: u8,
    rta_table: Option<u32>,
) -> bool {
    route_is_default_ipv4(family, dst_len, route_type)
        && rta_table.unwrap_or(rtm_table as u32) == RT_TABLE_MAIN_BITS
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
                let is_up = flags & IFF_UP_BITS != 0;
                let is_lower_up = interface_is_lower_up(flags);

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
        header: NlMsgHdr {
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
        let message = read_ifaddr_msg(payload)?;
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
                            let cache = read_ifa_cache_info(data)?;
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
        header: NlMsgHdr {
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
        let message = read_rt_msg(payload)?;

        let mut interface_index = None;
        let mut route_table = None;
        parse_rtattrs(&payload[std::mem::size_of::<RtMsg>()..], |kind, data| {
            if kind == libc::RTA_OIF {
                interface_index = read_u32(data);
            } else if kind == RTA_TABLE_BITS {
                route_table = read_u32(data);
            }
            Ok(())
        })?;

        if !route_is_main_default_ipv4(
            message.rtm_family,
            message.rtm_dst_len,
            message.rtm_type,
            message.rtm_table,
            route_table,
        ) {
            return Ok(());
        }

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
    header: NlMsgHdr,
    body: T,
}

#[cfg(any(target_os = "linux", test))]
#[derive(Clone, Copy)]
#[repr(C)]
struct NlMsgHdr {
    nlmsg_len: u32,
    nlmsg_type: u16,
    nlmsg_flags: u16,
    nlmsg_seq: u32,
    nlmsg_pid: u32,
}

#[cfg(any(target_os = "linux", test))]
#[derive(Clone, Copy)]
#[repr(C)]
struct NlMsgErr {
    error: i32,
    msg: NlMsgHdr,
}

#[cfg(any(target_os = "linux", test))]
#[derive(Clone, Copy)]
#[repr(C)]
struct IfAddrMsg {
    ifa_family: u8,
    ifa_prefixlen: u8,
    ifa_flags: u8,
    ifa_scope: u8,
    ifa_index: u32,
}

#[cfg(any(target_os = "linux", test))]
#[derive(Clone, Copy)]
#[repr(C)]
struct IfaCacheInfo {
    ifa_prefered: u32,
    ifa_valid: u32,
    cstamp: u32,
    tstamp: u32,
}

#[cfg(any(target_os = "linux", test))]
#[derive(Clone, Copy)]
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

#[cfg(any(target_os = "linux", test))]
#[derive(Clone, Copy)]
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
    if let Err(err) = set_netlink_recv_timeout(fd) {
        unsafe {
            libc::close(fd);
        }
        return Err(err);
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
fn set_netlink_recv_timeout(fd: libc::c_int) -> std::io::Result<()> {
    let timeout = netlink_recv_timeout_timeval();
    let rc = unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_RCVTIMEO,
            (&timeout as *const libc::timeval).cast(),
            std::mem::size_of::<libc::timeval>() as libc::socklen_t,
        )
    };
    if rc == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(any(target_os = "linux", test))]
fn netlink_recv_timeout_timeval() -> libc::timeval {
    libc::timeval {
        tv_sec: NETLINK_RECV_TIMEOUT.as_secs() as _,
        tv_usec: NETLINK_RECV_TIMEOUT.subsec_micros() as _,
    }
}

#[cfg(target_os = "linux")]
fn netlink_buffer_is_done(buffer: &[u8]) -> std::io::Result<bool> {
    let mut offset = 0usize;
    while offset + std::mem::size_of::<NlMsgHdr>() <= buffer.len() {
        let header = read_nlmsg_header(&buffer[offset..])?;
        if header.nlmsg_len < std::mem::size_of::<NlMsgHdr>() as u32 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid netlink header length",
            ));
        }
        if header.nlmsg_type == NLMSG_DONE_BITS {
            return Ok(true);
        }
        offset += nlmsg_align(header.nlmsg_len as usize);
    }
    Ok(false)
}

#[cfg(any(target_os = "linux", test))]
fn parse_netlink_messages<F>(buffer: &[u8], mut visit: F) -> std::io::Result<()>
where
    F: FnMut(u16, &[u8]) -> std::io::Result<()>,
{
    let mut offset = 0usize;
    while offset + std::mem::size_of::<NlMsgHdr>() <= buffer.len() {
        let header = read_nlmsg_header(&buffer[offset..])?;
        if header.nlmsg_len < std::mem::size_of::<NlMsgHdr>() as u32 {
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
        let payload = &buffer[offset + std::mem::size_of::<NlMsgHdr>()..end];
        match header.nlmsg_type {
            value if value == NLMSG_DONE_BITS => break,
            value if value == NLMSG_ERROR_BITS => {
                if payload.len() < std::mem::size_of::<NlMsgErr>() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "short netlink error payload",
                    ));
                }
                let message = read_nlmsg_error(payload)?;
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

#[cfg(any(target_os = "linux", test))]
fn parse_rtattrs<F>(payload: &[u8], mut visit: F) -> std::io::Result<()>
where
    F: FnMut(u16, &[u8]) -> std::io::Result<()>,
{
    let mut offset = 0usize;
    while offset + std::mem::size_of::<RtAttr>() <= payload.len() {
        let attr = read_rt_attr(&payload[offset..])?;
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

#[cfg(any(target_os = "linux", test))]
fn read_nlmsg_header(data: &[u8]) -> std::io::Result<NlMsgHdr> {
    read_unaligned_struct(data, "netlink header")
}

#[cfg(any(target_os = "linux", test))]
fn read_nlmsg_error(data: &[u8]) -> std::io::Result<NlMsgErr> {
    read_unaligned_struct(data, "netlink error")
}

#[cfg(any(target_os = "linux", test))]
fn read_ifaddr_msg(data: &[u8]) -> std::io::Result<IfAddrMsg> {
    read_unaligned_struct(data, "ifaddrmsg")
}

#[cfg(any(target_os = "linux", test))]
fn read_ifa_cache_info(data: &[u8]) -> std::io::Result<IfaCacheInfo> {
    read_unaligned_struct(data, "ifa cache info")
}

#[cfg(any(target_os = "linux", test))]
fn read_rt_msg(data: &[u8]) -> std::io::Result<RtMsg> {
    read_unaligned_struct(data, "rtmsg")
}

#[cfg(any(target_os = "linux", test))]
fn read_rt_attr(data: &[u8]) -> std::io::Result<RtAttr> {
    read_unaligned_struct(data, "rtattr")
}

#[cfg(any(target_os = "linux", test))]
fn read_unaligned_struct<T: Copy>(data: &[u8], name: &str) -> std::io::Result<T> {
    if data.len() < std::mem::size_of::<T>() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("short {name}"),
        ));
    }
    Ok(unsafe { std::ptr::read_unaligned(data.as_ptr().cast::<T>()) })
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
    let mut name = [0 as libc::c_char; libc::IF_NAMESIZE];
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

#[cfg(any(target_os = "linux", test))]
fn read_u32(data: &[u8]) -> Option<u32> {
    if data.len() < std::mem::size_of::<u32>() {
        return None;
    }
    Some(u32::from_ne_bytes([data[0], data[1], data[2], data[3]]))
}

#[cfg(any(target_os = "linux", test))]
fn nlmsg_align(len: usize) -> usize {
    (len + 3) & !3
}

#[cfg(any(target_os = "linux", test))]
fn rta_align(len: usize) -> usize {
    (len + 3) & !3
}

#[cfg(test)]
mod tests {
    use super::*;

    fn append_struct<T: Copy>(out: &mut Vec<u8>, value: &T) {
        let bytes = unsafe {
            std::slice::from_raw_parts((value as *const T).cast::<u8>(), std::mem::size_of::<T>())
        };
        out.extend_from_slice(bytes);
    }

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
    fn netlink_enrichment_errors_keep_collected_addresses() {
        let snapshot = collect_network_snapshot_best_effort(
            |snapshot| {
                snapshot
                    .addresses
                    .push(addr("eth0", [192, 168, 55, 20], false));
                Ok(())
            },
            |_snapshot| Err(std::io::Error::other("addr netlink stalled")),
            |_snapshot| Err(std::io::Error::other("route netlink stalled")),
        )
        .unwrap();

        assert_eq!(
            select_ipv4(&snapshot, &SelectionConfig::default()),
            Selection::Show(Ipv4Addr::new(192, 168, 55, 20))
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

    #[test]
    fn lower_up_requires_running_flag_when_available() {
        let up_non_loopback = IFF_UP_BITS;
        assert!(!interface_is_lower_up(up_non_loopback));
        assert!(interface_is_lower_up(IFF_UP_BITS | IFF_RUNNING_BITS));
    }

    #[test]
    fn default_route_helper_accepts_only_main_table_routes() {
        assert!(route_is_main_default_ipv4(
            AF_INET_BITS,
            0,
            RTN_UNICAST_BITS,
            RT_TABLE_MAIN_BITS as u8,
            None,
        ));
        assert!(!route_is_main_default_ipv4(
            AF_INET_BITS,
            0,
            RTN_UNICAST_BITS,
            100,
            None,
        ));
        assert!(!route_is_main_default_ipv4(
            AF_INET_BITS,
            0,
            RTN_UNICAST_BITS,
            RT_TABLE_MAIN_BITS as u8,
            Some(100),
        ));
        assert!(route_is_main_default_ipv4(
            AF_INET_BITS,
            0,
            RTN_UNICAST_BITS,
            100,
            Some(RT_TABLE_MAIN_BITS),
        ));
    }

    #[test]
    fn netlink_message_parser_reads_unaligned_headers() {
        let payload = [7, 8, 9];
        let header = NlMsgHdr {
            nlmsg_len: (std::mem::size_of::<NlMsgHdr>() + payload.len()) as u32,
            nlmsg_type: 99,
            nlmsg_flags: 0,
            nlmsg_seq: 1,
            nlmsg_pid: 0,
        };
        let done = NlMsgHdr {
            nlmsg_len: std::mem::size_of::<NlMsgHdr>() as u32,
            nlmsg_type: NLMSG_DONE_BITS,
            nlmsg_flags: 0,
            nlmsg_seq: 1,
            nlmsg_pid: 0,
        };
        let mut buffer = vec![0xaa];
        append_struct(&mut buffer, &header);
        buffer.extend_from_slice(&payload);
        append_struct(&mut buffer, &done);

        let mut seen = Vec::new();
        parse_netlink_messages(&buffer[1..], |message_type, data| {
            seen.push((message_type, data.to_vec()));
            Ok(())
        })
        .unwrap();

        assert_eq!(seen, vec![(99, payload.to_vec())]);
    }

    #[test]
    fn netlink_message_parser_reads_unaligned_errors() {
        let error = NlMsgErr {
            error: -22,
            msg: NlMsgHdr {
                nlmsg_len: std::mem::size_of::<NlMsgHdr>() as u32,
                nlmsg_type: 0,
                nlmsg_flags: 0,
                nlmsg_seq: 1,
                nlmsg_pid: 0,
            },
        };
        let header = NlMsgHdr {
            nlmsg_len: (std::mem::size_of::<NlMsgHdr>() + std::mem::size_of::<NlMsgErr>()) as u32,
            nlmsg_type: NLMSG_ERROR_BITS,
            nlmsg_flags: 0,
            nlmsg_seq: 1,
            nlmsg_pid: 0,
        };
        let mut buffer = vec![0xaa];
        append_struct(&mut buffer, &header);
        append_struct(&mut buffer, &error);

        let err = parse_netlink_messages(&buffer[1..], |_message_type, _data| Ok(()))
            .expect_err("negative netlink error should be returned");

        assert_eq!(err.raw_os_error(), Some(22));
    }

    #[test]
    fn route_attribute_parser_reads_unaligned_attributes() {
        let attr = RtAttr {
            rta_len: (std::mem::size_of::<RtAttr>() + std::mem::size_of::<u32>()) as u16,
            rta_type: RTA_TABLE_BITS,
        };
        let mut buffer = vec![0xaa];
        append_struct(&mut buffer, &attr);
        buffer.extend_from_slice(&RT_TABLE_MAIN_BITS.to_ne_bytes());

        let mut seen = Vec::new();
        parse_rtattrs(&buffer[1..], |kind, data| {
            seen.push((kind, read_u32(data).unwrap()));
            Ok(())
        })
        .unwrap();

        assert_eq!(seen, vec![(RTA_TABLE_BITS, RT_TABLE_MAIN_BITS)]);
    }

    #[test]
    fn netlink_body_readers_copy_from_unaligned_slices() {
        let ifaddr = IfAddrMsg {
            ifa_family: AF_INET_BITS,
            ifa_prefixlen: 24,
            ifa_flags: IFA_F_PERMANENT_BITS as u8,
            ifa_scope: 0,
            ifa_index: 3,
        };
        let cache = IfaCacheInfo {
            ifa_prefered: 30,
            ifa_valid: INFINITE_LIFETIME,
            cstamp: 1,
            tstamp: 2,
        };
        let route = RtMsg {
            rtm_family: AF_INET_BITS,
            rtm_dst_len: 0,
            rtm_src_len: 0,
            rtm_tos: 0,
            rtm_table: RT_TABLE_MAIN_BITS as u8,
            rtm_protocol: 0,
            rtm_scope: 0,
            rtm_type: RTN_UNICAST_BITS,
            rtm_flags: 0,
        };

        let mut ifaddr_buf = vec![0xaa];
        append_struct(&mut ifaddr_buf, &ifaddr);
        let mut cache_buf = vec![0xaa];
        append_struct(&mut cache_buf, &cache);
        let mut route_buf = vec![0xaa];
        append_struct(&mut route_buf, &route);

        assert_eq!(read_ifaddr_msg(&ifaddr_buf[1..]).unwrap().ifa_index, 3);
        assert_eq!(
            read_ifa_cache_info(&cache_buf[1..]).unwrap().ifa_prefered,
            30
        );
        assert_eq!(
            read_rt_msg(&route_buf[1..]).unwrap().rtm_table,
            RT_TABLE_MAIN_BITS as u8
        );
    }

    #[test]
    fn netlink_recv_timeout_is_subsecond_and_nonzero() {
        let timeout = netlink_recv_timeout_timeval();

        assert_eq!(timeout.tv_sec, 0);
        assert!(timeout.tv_usec > 0);
        assert!(timeout.tv_usec <= 200_000);
    }
}
