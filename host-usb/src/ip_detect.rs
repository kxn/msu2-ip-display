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
            .find(|candidate| candidate.interface == *interface && is_normal_ipv4(candidate.address))
            .map(|candidate| Selection::Show(candidate.address))
            .unwrap_or_else(|| {
                if candidates
                    .iter()
                    .any(|candidate| candidate.interface == *interface && is_link_local(candidate.address))
                {
                    Selection::FailureCandidate
                } else {
                    Selection::Pending
                }
            });
    }

    if let Some(default_route) = snapshot.routes.iter().find(|route| route.is_default) {
        if let Some(candidate) = candidates
            .iter()
            .copied()
            .find(|candidate| candidate.interface == default_route.interface && is_normal_ipv4(candidate.address))
        {
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
            if candidates
                .iter()
                .any(|candidate| !is_virtual_interface(&candidate.interface) && is_link_local(candidate.address))
            {
                Selection::FailureCandidate
            } else {
                Selection::Pending
            }
        }
        1 => Selection::Show(normal[0].address),
        _ => {
            let dynamic: Vec<&AddressCandidate> =
                normal.iter().copied().filter(|candidate| candidate.is_dynamic).collect();
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
            addresses: vec![addr("eth0", [192, 168, 1, 10], true), addr("eth1", [10, 0, 0, 5], true)],
            routes: vec![Route { interface: "eth1".to_string(), is_default: true }],
        };
        let config = SelectionConfig { interface: Some("eth0".to_string()) };

        assert_eq!(select_ipv4(&snapshot, &config), Selection::Show(Ipv4Addr::new(192, 168, 1, 10)));
    }

    #[test]
    fn default_route_interface_is_preferred() {
        let snapshot = NetworkSnapshot {
            addresses: vec![addr("eth0", [192, 168, 1, 10], true), addr("eth1", [10, 0, 0, 5], true)],
            routes: vec![Route { interface: "eth1".to_string(), is_default: true }],
        };

        assert_eq!(select_ipv4(&snapshot, &SelectionConfig::default()), Selection::Show(Ipv4Addr::new(10, 0, 0, 5)));
    }

    #[test]
    fn isolated_single_normal_ipv4_is_displayed() {
        let snapshot = NetworkSnapshot {
            addresses: vec![addr("eth0", [192, 168, 55, 20], true)],
            routes: vec![],
        };

        assert_eq!(select_ipv4(&snapshot, &SelectionConfig::default()), Selection::Show(Ipv4Addr::new(192, 168, 55, 20)));
    }

    #[test]
    fn multiple_without_default_prefers_dynamic() {
        let snapshot = NetworkSnapshot {
            addresses: vec![addr("eth0", [192, 168, 1, 10], false), addr("eth1", [10, 0, 0, 5], true)],
            routes: vec![],
        };

        assert_eq!(select_ipv4(&snapshot, &SelectionConfig::default()), Selection::Show(Ipv4Addr::new(10, 0, 0, 5)));
    }

    #[test]
    fn link_local_only_is_failure_candidate_after_delay() {
        let snapshot = NetworkSnapshot {
            addresses: vec![addr("eth0", [169, 254, 1, 2], true)],
            routes: vec![],
        };

        assert_eq!(select_ipv4(&snapshot, &SelectionConfig::default()), Selection::FailureCandidate);
    }

    #[test]
    fn virtual_interfaces_are_ignored_for_fallback() {
        let snapshot = NetworkSnapshot {
            addresses: vec![addr("docker0", [172, 17, 0, 1], false)],
            routes: vec![],
        };

        assert_eq!(select_ipv4(&snapshot, &SelectionConfig::default()), Selection::Pending);
    }

    #[test]
    fn virtual_link_local_only_is_pending_for_fallback() {
        let snapshot = NetworkSnapshot {
            addresses: vec![addr("docker0", [169, 254, 1, 2], true)],
            routes: vec![],
        };

        assert_eq!(select_ipv4(&snapshot, &SelectionConfig::default()), Selection::Pending);
    }
}
