use std::net::Ipv4Addr;
use std::time::Instant;

use crate::cli::RunOptions;
use crate::ip_detect::{NetworkSnapshot, Selection, SelectionConfig};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonState {
    Listening,
    Connecting,
    ConnectedPendingIp,
    ConnectedDhcpFailed,
    ConnectedShowingIp(Ipv4Addr),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaemonEvent {
    DeviceCandidateFound,
    HandshakeOk,
    DeviceDisconnected,
    NetworkSnapshot {
        snapshot: NetworkSnapshot,
        now: Instant,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaemonAction {
    OpenDevice,
    CloseDevice,
    ShowPending,
    ShowDhcpFailed,
    ShowIp(Ipv4Addr),
}

pub struct Daemon {
    pub state: DaemonState,
    options: RunOptions,
    failure_since: Option<Instant>,
}

impl Daemon {
    pub fn new(options: RunOptions) -> Self {
        Self {
            state: DaemonState::Listening,
            options,
            failure_since: None,
        }
    }

    pub fn handle_event(&mut self, event: DaemonEvent) -> Vec<DaemonAction> {
        match event {
            DaemonEvent::DeviceCandidateFound if self.state == DaemonState::Listening => {
                self.state = DaemonState::Connecting;
                vec![DaemonAction::OpenDevice]
            }
            DaemonEvent::HandshakeOk if self.state == DaemonState::Connecting => {
                self.state = DaemonState::ConnectedPendingIp;
                self.failure_since = None;
                vec![DaemonAction::ShowPending]
            }
            DaemonEvent::DeviceDisconnected => {
                self.state = DaemonState::Listening;
                self.failure_since = None;
                vec![DaemonAction::CloseDevice]
            }
            DaemonEvent::NetworkSnapshot { snapshot, now } if is_connected(self.state) => {
                let config = SelectionConfig {
                    interface: self.options.interface.clone(),
                };

                match crate::ip_detect::select_ipv4(&snapshot, &config) {
                    Selection::Show(ip) => {
                        self.failure_since = None;
                        if self.state == DaemonState::ConnectedShowingIp(ip) {
                            Vec::new()
                        } else {
                            self.state = DaemonState::ConnectedShowingIp(ip);
                            vec![DaemonAction::ShowIp(ip)]
                        }
                    }
                    Selection::Pending => {
                        self.failure_since = None;
                        if self.state == DaemonState::ConnectedPendingIp {
                            Vec::new()
                        } else {
                            self.state = DaemonState::ConnectedPendingIp;
                            vec![DaemonAction::ShowPending]
                        }
                    }
                    Selection::FailureCandidate => {
                        if let Some(first_seen) = self.failure_since {
                            if now.duration_since(first_seen) >= self.options.dhcp_fail_delay {
                                if self.state == DaemonState::ConnectedDhcpFailed {
                                    Vec::new()
                                } else {
                                    self.state = DaemonState::ConnectedDhcpFailed;
                                    vec![DaemonAction::ShowDhcpFailed]
                                }
                            } else {
                                Vec::new()
                            }
                        } else {
                            self.failure_since = Some(now);
                            Vec::new()
                        }
                    }
                }
            }
            _ => Vec::new(),
        }
    }
}

fn is_connected(state: DaemonState) -> bool {
    matches!(
        state,
        DaemonState::ConnectedPendingIp
            | DaemonState::ConnectedDhcpFailed
            | DaemonState::ConnectedShowingIp(_)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{DisplayMode, ResourceMode};
    use crate::ip_detect::{AddressCandidate, Route};
    use std::time::Duration;

    fn options() -> RunOptions {
        RunOptions {
            interface: None,
            dhcp_fail_delay: Duration::from_secs(45),
            resources: ResourceMode::Flashed,
            debug: false,
            show: DisplayMode::Text,
        }
    }

    fn snapshot(address: [u8; 4]) -> NetworkSnapshot {
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

    #[test]
    fn connect_success_shows_pending_page() {
        let mut daemon = Daemon::new(options());
        assert_eq!(
            daemon.handle_event(DaemonEvent::DeviceCandidateFound),
            vec![DaemonAction::OpenDevice]
        );
        assert_eq!(daemon.state, DaemonState::Connecting);

        assert_eq!(
            daemon.handle_event(DaemonEvent::HandshakeOk),
            vec![DaemonAction::ShowPending]
        );
        assert_eq!(daemon.state, DaemonState::ConnectedPendingIp);
    }

    #[test]
    fn displayable_ip_switches_to_showing_ip() {
        let mut daemon = Daemon::new(options());
        daemon.handle_event(DaemonEvent::DeviceCandidateFound);
        daemon.handle_event(DaemonEvent::HandshakeOk);
        let now = Instant::now();

        assert_eq!(
            daemon.handle_event(DaemonEvent::NetworkSnapshot {
                snapshot: snapshot([192, 168, 1, 20]),
                now,
            }),
            vec![DaemonAction::ShowIp(Ipv4Addr::new(192, 168, 1, 20))]
        );
        assert_eq!(
            daemon.state,
            DaemonState::ConnectedShowingIp(Ipv4Addr::new(192, 168, 1, 20))
        );
    }

    #[test]
    fn link_local_waits_for_failure_delay_before_failure_page() {
        let mut daemon = Daemon::new(options());
        daemon.handle_event(DaemonEvent::DeviceCandidateFound);
        daemon.handle_event(DaemonEvent::HandshakeOk);
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

        assert_eq!(
            daemon.handle_event(DaemonEvent::NetworkSnapshot {
                snapshot: link_local.clone(),
                now: start,
            }),
            Vec::<DaemonAction>::new()
        );
        assert_eq!(
            daemon.handle_event(DaemonEvent::NetworkSnapshot {
                snapshot: link_local,
                now: start + Duration::from_secs(45),
            }),
            vec![DaemonAction::ShowDhcpFailed]
        );
        assert_eq!(daemon.state, DaemonState::ConnectedDhcpFailed);
    }

    #[test]
    fn disconnect_closes_device_and_returns_to_listening() {
        let mut daemon = Daemon::new(options());
        daemon.handle_event(DaemonEvent::DeviceCandidateFound);
        daemon.handle_event(DaemonEvent::HandshakeOk);

        assert_eq!(
            daemon.handle_event(DaemonEvent::DeviceDisconnected),
            vec![DaemonAction::CloseDevice]
        );
        assert_eq!(daemon.state, DaemonState::Listening);
    }
}
