#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitKind {
    Systemd,
    OpenRc,
    OpenWrtProcd,
    SysV,
    BusyBox,
    RunitTemplate,
    S6Template,
    DinitTemplate,
    Unknown,
}

#[derive(Debug, Clone, Default)]
pub struct InitProbe {
    pub has_openwrt_release: bool,
    pub has_procd: bool,
    pub has_systemd_runtime: bool,
    pub has_systemctl: bool,
    pub has_openrc_runtime: bool,
    pub has_rc_service: bool,
    pub has_update_rc_d: bool,
    pub has_chkconfig: bool,
    pub pid1_comm: Option<String>,
    pub has_busybox: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallSpec {
    pub binary_path: String,
    pub service_args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceRender {
    pub path: String,
    pub contents: String,
}

pub fn detect_init(probe: &InitProbe) -> InitKind {
    if probe.has_openwrt_release || probe.has_procd {
        InitKind::OpenWrtProcd
    } else if probe.has_systemd_runtime && probe.has_systemctl {
        InitKind::Systemd
    } else if probe.has_openrc_runtime || probe.has_rc_service {
        InitKind::OpenRc
    } else if probe.has_update_rc_d || probe.has_chkconfig {
        InitKind::SysV
    } else if probe.pid1_comm.as_deref() == Some("init") && probe.has_busybox {
        InitKind::BusyBox
    } else {
        InitKind::Unknown
    }
}

pub fn render_service(kind: InitKind, spec: &InstallSpec) -> ServiceRender {
    match kind {
        InitKind::Systemd => render_systemd(spec),
        InitKind::OpenWrtProcd => render_openwrt(spec),
        InitKind::OpenRc => render_openrc(spec),
        InitKind::SysV => render_sysv(spec),
        InitKind::BusyBox => render_busybox(spec),
        InitKind::RunitTemplate => render_template("/etc/sv/miniboard-ipd/run", spec),
        InitKind::S6Template => render_template("s6-miniboard-ipd-run", spec),
        InitKind::DinitTemplate => render_template("miniboard-ipd.dinit", spec),
        InitKind::Unknown => render_template("miniboard-ipd.manual", spec),
    }
}

fn command_line(spec: &InstallSpec) -> String {
    let mut parts = vec![spec.binary_path.clone(), "run".to_string()];
    parts.extend(spec.service_args.clone());
    parts.join(" ")
}

fn command_args(spec: &InstallSpec) -> String {
    let mut parts = vec!["run".to_string()];
    parts.extend(spec.service_args.clone());
    parts.join(" ")
}

fn render_systemd(spec: &InstallSpec) -> ServiceRender {
    ServiceRender {
        path: "/etc/systemd/system/miniboard-ipd.service".to_string(),
        contents: format!(
            "[Unit]\nDescription=Miniboard IP display daemon\nAfter=network.target\n\n[Service]\nType=simple\nExecStart={}\nRestart=always\nRestartSec=2\n\n[Install]\nWantedBy=multi-user.target\n",
            command_line(spec)
        ),
    }
}

fn render_openwrt(spec: &InstallSpec) -> ServiceRender {
    ServiceRender {
        path: "/etc/init.d/miniboard-ipd".to_string(),
        contents: format!(
            "#!/bin/sh /etc/rc.common\nSTART=95\nUSE_PROCD=1\n\nstart_service() {{\n\tprocd_open_instance\n\tprocd_set_param command {}\n\tprocd_set_param respawn\n\tprocd_close_instance\n}}\n",
            command_line(spec)
        ),
    }
}

fn render_openrc(spec: &InstallSpec) -> ServiceRender {
    ServiceRender {
        path: "/etc/init.d/miniboard-ipd".to_string(),
        contents: format!(
            "#!/sbin/openrc-run\ncommand=\"{}\"\ncommand_args=\"{}\"\ncommand_background=true\npidfile=\"/run/miniboard-ipd.pid\"\ndepend() {{\n\tneed localmount\n\tafter net\n}}\n",
            spec.binary_path,
            command_args(spec)
        ),
    }
}

fn render_sysv(spec: &InstallSpec) -> ServiceRender {
    ServiceRender {
        path: "/etc/init.d/miniboard-ipd".to_string(),
        contents: format!(
            "#!/bin/sh\n### BEGIN INIT INFO\n# Provides: miniboard-ipd\n# Required-Start: $local_fs $network\n# Required-Stop: $local_fs\n# Default-Start: 2 3 4 5\n# Default-Stop: 0 1 6\n# Short-Description: Miniboard IP display daemon\n### END INIT INFO\ncase \"$1\" in\n  start)\n    {} &\n    ;;\n  stop)\n    pkill -f \"{} run\" || true\n    ;;\n  restart)\n    \"$0\" stop\n    \"$0\" start\n    ;;\n  *)\n    echo \"Usage: $0 {{start|stop|restart}}\"\n    exit 1\n    ;;\nesac\n",
            command_line(spec),
            spec.binary_path
        ),
    }
}

fn render_busybox(spec: &InstallSpec) -> ServiceRender {
    ServiceRender {
        path: "/etc/init.d/S99miniboard-ipd".to_string(),
        contents: format!(
            "#!/bin/sh\ncase \"$1\" in\n  start) {} & ;;\n  stop) pkill -f \"{} run\" || true ;;\nesac\n",
            command_line(spec),
            spec.binary_path
        ),
    }
}

fn render_template(path: &str, spec: &InstallSpec) -> ServiceRender {
    ServiceRender {
        path: path.to_string(),
        contents: format!("#!/bin/sh\nexec {}\n", command_line(spec)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> InstallSpec {
        InstallSpec {
            binary_path: "/usr/local/bin/miniboard-ipd".to_string(),
            service_args: vec![
                "--interface".to_string(),
                "eth0".to_string(),
                "--dhcp-fail-delay-seconds".to_string(),
                "45".to_string(),
            ],
        }
    }

    #[test]
    fn openwrt_detection_wins_before_systemd() {
        let probe = InitProbe {
            has_openwrt_release: true,
            has_procd: true,
            has_systemd_runtime: true,
            has_systemctl: true,
            ..InitProbe::default()
        };
        assert_eq!(detect_init(&probe), InitKind::OpenWrtProcd);
    }

    #[test]
    fn systemd_unit_embeds_install_arguments() {
        let render = render_service(InitKind::Systemd, &spec());
        assert_eq!(render.path, "/etc/systemd/system/miniboard-ipd.service");
        assert!(render.contents.contains(
            "ExecStart=/usr/local/bin/miniboard-ipd run --interface eth0 --dhcp-fail-delay-seconds 45"
        ));
        assert!(render.contents.contains("WantedBy=multi-user.target"));
    }

    #[test]
    fn openwrt_script_uses_procd_respawn() {
        let render = render_service(InitKind::OpenWrtProcd, &spec());
        assert_eq!(render.path, "/etc/init.d/miniboard-ipd");
        assert!(render.contents.contains("USE_PROCD=1"));
        assert!(render.contents.contains(
            "procd_set_param command /usr/local/bin/miniboard-ipd run --interface eth0 --dhcp-fail-delay-seconds 45"
        ));
        assert!(render.contents.contains("procd_set_param respawn"));
    }

    #[test]
    fn openrc_script_uses_command_args() {
        let render = render_service(InitKind::OpenRc, &spec());
        assert_eq!(render.path, "/etc/init.d/miniboard-ipd");
        assert!(render.contents.contains("command=\"/usr/local/bin/miniboard-ipd\""));
        assert!(render.contents.contains("command_args=\"run --interface eth0 --dhcp-fail-delay-seconds 45\""));
    }
}
