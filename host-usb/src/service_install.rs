use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitKind {
    Systemd,
    OpenRc,
    OpenWrtProcd,
    SysV,
    SysVUpdateRcD,
    SysVChkconfig,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallRequest {
    pub source_binary_path: PathBuf,
    pub spec: InstallSpec,
    pub init: InitKind,
}

pub trait InstallOps {
    fn copy_file(&mut self, from: &Path, to: &str, executable: bool) -> std::io::Result<()>;
    fn write_file(&mut self, path: &str, contents: &str, executable: bool) -> std::io::Result<()>;
    fn remove_file(&mut self, path: &str) -> std::io::Result<()>;
    fn run(&mut self, command: &[String]) -> std::io::Result<()>;
}

pub fn detect_init(probe: &InitProbe) -> InitKind {
    if probe.has_openwrt_release || probe.has_procd {
        InitKind::OpenWrtProcd
    } else if probe.has_systemd_runtime && probe.has_systemctl {
        InitKind::Systemd
    } else if probe.has_openrc_runtime || probe.has_rc_service {
        InitKind::OpenRc
    } else if probe.has_update_rc_d {
        InitKind::SysVUpdateRcD
    } else if probe.has_chkconfig {
        InitKind::SysVChkconfig
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
        InitKind::SysV | InitKind::SysVUpdateRcD | InitKind::SysVChkconfig => render_sysv(spec),
        InitKind::BusyBox => render_busybox(spec),
        InitKind::RunitTemplate => render_template("/etc/sv/miniboard-ipd/run", spec),
        InitKind::S6Template => render_template("s6-miniboard-ipd-run", spec),
        InitKind::DinitTemplate => render_template("miniboard-ipd.dinit", spec),
        InitKind::Unknown => render_template("miniboard-ipd.manual", spec),
    }
}

pub fn install_commands(kind: InitKind) -> Vec<Vec<String>> {
    match kind {
        InitKind::Systemd => vec![
            vec!["systemctl".into(), "daemon-reload".into()],
            vec![
                "systemctl".into(),
                "enable".into(),
                "--now".into(),
                "miniboard-ipd.service".into(),
            ],
        ],
        InitKind::OpenRc => vec![
            vec![
                "rc-update".into(),
                "add".into(),
                "miniboard-ipd".into(),
                "default".into(),
            ],
            vec!["rc-service".into(), "miniboard-ipd".into(), "start".into()],
        ],
        InitKind::OpenWrtProcd => vec![
            vec!["/etc/init.d/miniboard-ipd".into(), "enable".into()],
            vec!["/etc/init.d/miniboard-ipd".into(), "start".into()],
        ],
        InitKind::SysV => vec![vec![
            "service".into(),
            "miniboard-ipd".into(),
            "start".into(),
        ]],
        InitKind::SysVUpdateRcD => vec![
            vec![
                "update-rc.d".into(),
                "miniboard-ipd".into(),
                "defaults".into(),
            ],
            vec!["service".into(), "miniboard-ipd".into(), "start".into()],
        ],
        InitKind::SysVChkconfig => vec![
            vec!["chkconfig".into(), "--add".into(), "miniboard-ipd".into()],
            vec!["service".into(), "miniboard-ipd".into(), "start".into()],
        ],
        InitKind::BusyBox => vec![vec!["/etc/init.d/S99miniboard-ipd".into(), "start".into()]],
        _ => Vec::new(),
    }
}

pub fn uninstall_commands(kind: InitKind) -> Vec<Vec<String>> {
    match kind {
        InitKind::Systemd => vec![vec![
            "systemctl".into(),
            "disable".into(),
            "--now".into(),
            "miniboard-ipd.service".into(),
        ]],
        InitKind::OpenRc => vec![
            vec!["rc-service".into(), "miniboard-ipd".into(), "stop".into()],
            vec![
                "rc-update".into(),
                "del".into(),
                "miniboard-ipd".into(),
                "default".into(),
            ],
        ],
        InitKind::OpenWrtProcd => vec![
            vec!["/etc/init.d/miniboard-ipd".into(), "stop".into()],
            vec!["/etc/init.d/miniboard-ipd".into(), "disable".into()],
        ],
        InitKind::SysV => vec![vec![
            "service".into(),
            "miniboard-ipd".into(),
            "stop".into(),
        ]],
        InitKind::SysVUpdateRcD => vec![
            vec!["service".into(), "miniboard-ipd".into(), "stop".into()],
            vec![
                "update-rc.d".into(),
                "miniboard-ipd".into(),
                "remove".into(),
            ],
        ],
        InitKind::SysVChkconfig => vec![
            vec!["service".into(), "miniboard-ipd".into(), "stop".into()],
            vec!["chkconfig".into(), "--del".into(), "miniboard-ipd".into()],
        ],
        InitKind::BusyBox => vec![vec!["/etc/init.d/S99miniboard-ipd".into(), "stop".into()]],
        _ => Vec::new(),
    }
}

pub fn post_uninstall_commands(kind: InitKind) -> Vec<Vec<String>> {
    match kind {
        InitKind::Systemd => vec![vec!["systemctl".into(), "daemon-reload".into()]],
        _ => Vec::new(),
    }
}

pub fn status_command(kind: InitKind) -> Vec<String> {
    match kind {
        InitKind::Systemd => vec![
            "systemctl".into(),
            "status".into(),
            "--no-pager".into(),
            "miniboard-ipd.service".into(),
        ],
        InitKind::OpenRc => vec!["rc-service".into(), "miniboard-ipd".into(), "status".into()],
        InitKind::OpenWrtProcd => vec!["/etc/init.d/miniboard-ipd".into(), "status".into()],
        InitKind::SysV | InitKind::SysVUpdateRcD | InitKind::SysVChkconfig => {
            vec!["service".into(), "miniboard-ipd".into(), "status".into()]
        }
        InitKind::BusyBox => vec!["pgrep".into(), "-af".into(), "miniboard-ipd".into()],
        _ => vec!["pgrep".into(), "-af".into(), "miniboard-ipd".into()],
    }
}

pub fn apply_install(_request: &InstallRequest, _ops: &mut dyn InstallOps) -> std::io::Result<()> {
    let request = _request;
    let ops = _ops;
    let rendered = render_service(request.init, &request.spec);
    ops.copy_file(&request.source_binary_path, &request.spec.binary_path, true)?;
    ops.write_file(
        &rendered.path,
        &rendered.contents,
        service_needs_executable(request.init),
    )?;
    for command in install_commands(request.init) {
        ops.run(&command)?;
    }
    Ok(())
}

pub fn apply_uninstall(
    kind: InitKind,
    binary_path: &str,
    ops: &mut dyn InstallOps,
) -> std::io::Result<()> {
    for command in uninstall_commands(kind) {
        ops.run(&command)?;
    }
    let rendered = render_service(
        kind,
        &InstallSpec {
            binary_path: binary_path.to_string(),
            service_args: Vec::new(),
        },
    );
    ops.remove_file(&rendered.path)?;
    ops.remove_file(binary_path)?;
    for command in post_uninstall_commands(kind) {
        ops.run(&command)?;
    }
    Ok(())
}

pub fn run_status(kind: InitKind, ops: &mut dyn InstallOps) -> std::io::Result<()> {
    ops.run(&status_command(kind))
}

fn service_needs_executable(kind: InitKind) -> bool {
    matches!(
        kind,
        InitKind::OpenRc
            | InitKind::OpenWrtProcd
            | InitKind::SysV
            | InitKind::SysVUpdateRcD
            | InitKind::SysVChkconfig
            | InitKind::BusyBox
    )
}

pub struct RealInstallOps;

impl InstallOps for RealInstallOps {
    fn copy_file(&mut self, from: &Path, to: &str, executable: bool) -> std::io::Result<()> {
        std::fs::copy(from, to)?;
        set_executable_if_needed(to, executable)
    }

    fn write_file(&mut self, path: &str, contents: &str, executable: bool) -> std::io::Result<()> {
        std::fs::write(path, contents)?;
        set_executable_if_needed(path, executable)
    }

    fn remove_file(&mut self, path: &str) -> std::io::Result<()> {
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err),
        }
    }

    fn run(&mut self, command: &[String]) -> std::io::Result<()> {
        let Some((program, args)) = command.split_first() else {
            return Ok(());
        };
        let status = std::process::Command::new(program).args(args).status()?;
        if status.success() {
            Ok(())
        } else {
            Err(std::io::Error::other(format!(
                "{program} exited with {status}"
            )))
        }
    }
}

#[cfg(target_family = "unix")]
fn set_executable_if_needed(path: &str, executable: bool) -> std::io::Result<()> {
    if !executable {
        return Ok(());
    }
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions)
}

#[cfg(not(target_family = "unix"))]
fn set_executable_if_needed(_path: &str, _executable: bool) -> std::io::Result<()> {
    Ok(())
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
            "#!/bin/sh\n### BEGIN INIT INFO\n# Provides: miniboard-ipd\n# Required-Start: $local_fs $network\n# Required-Stop: $local_fs\n# Default-Start: 2 3 4 5\n# Default-Stop: 0 1 6\n# Short-Description: Miniboard IP display daemon\n### END INIT INFO\ncase \"$1\" in\n  start)\n    {} &\n    ;;\n  stop)\n    pkill -f \"{} run\" || true\n    ;;\n  restart)\n    \"$0\" stop\n    \"$0\" start\n    ;;\n  status)\n    pgrep -af \"{} run\"\n    ;;\n  *)\n    echo \"Usage: $0 {{start|stop|restart|status}}\"\n    exit 1\n    ;;\nesac\n",
            command_line(spec),
            spec.binary_path,
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
    fn sysv_detection_preserves_update_rc_d_backend() {
        let probe = InitProbe {
            has_update_rc_d: true,
            ..InitProbe::default()
        };
        assert_eq!(detect_init(&probe), InitKind::SysVUpdateRcD);
    }

    #[test]
    fn sysv_detection_preserves_chkconfig_backend() {
        let probe = InitProbe {
            has_chkconfig: true,
            ..InitProbe::default()
        };
        assert_eq!(detect_init(&probe), InitKind::SysVChkconfig);
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
        assert!(render
            .contents
            .contains("command=\"/usr/local/bin/miniboard-ipd\""));
        assert!(render
            .contents
            .contains("command_args=\"run --interface eth0 --dhcp-fail-delay-seconds 45\""));
    }

    #[test]
    fn sysv_script_supports_status_command() {
        let render = render_service(InitKind::SysV, &spec());
        assert!(render.contents.contains("  status)"));
        assert!(render.contents.contains("pgrep -af"));
        assert!(render
            .contents
            .contains("Usage: $0 {start|stop|restart|status}"));
    }
}

#[cfg(test)]
mod command_tests {
    use super::*;

    #[derive(Default)]
    struct RecordingOps {
        events: Vec<String>,
    }

    impl InstallOps for RecordingOps {
        fn copy_file(&mut self, from: &Path, to: &str, executable: bool) -> std::io::Result<()> {
            self.events
                .push(format!("copy:{}:{to}:{executable}", from.display()));
            Ok(())
        }

        fn write_file(
            &mut self,
            path: &str,
            contents: &str,
            executable: bool,
        ) -> std::io::Result<()> {
            self.events.push(format!(
                "write:{path}:{executable}:{}",
                contents.contains("miniboard-ipd run")
            ));
            Ok(())
        }

        fn remove_file(&mut self, path: &str) -> std::io::Result<()> {
            self.events.push(format!("remove:{path}"));
            Ok(())
        }

        fn run(&mut self, command: &[String]) -> std::io::Result<()> {
            self.events.push(format!("run:{}", command.join(" ")));
            Ok(())
        }
    }

    fn request(kind: InitKind) -> InstallRequest {
        InstallRequest {
            source_binary_path: PathBuf::from("/tmp/miniboard-ipd"),
            spec: InstallSpec {
                binary_path: "/usr/local/bin/miniboard-ipd".to_string(),
                service_args: vec!["--interface".to_string(), "eth0".to_string()],
            },
            init: kind,
        }
    }

    #[test]
    fn systemd_install_commands_reload_enable_and_start() {
        assert_eq!(
            install_commands(InitKind::Systemd),
            vec![
                vec!["systemctl", "daemon-reload"],
                vec!["systemctl", "enable", "--now", "miniboard-ipd.service"],
            ]
        );
    }

    #[test]
    fn sysv_update_rc_d_install_enables_boot_start_and_starts_service() {
        assert_eq!(
            install_commands(InitKind::SysVUpdateRcD),
            vec![
                vec!["update-rc.d", "miniboard-ipd", "defaults"],
                vec!["service", "miniboard-ipd", "start"],
            ]
        );
    }

    #[test]
    fn sysv_chkconfig_install_enables_boot_start_and_starts_service() {
        assert_eq!(
            install_commands(InitKind::SysVChkconfig),
            vec![
                vec!["chkconfig", "--add", "miniboard-ipd"],
                vec!["service", "miniboard-ipd", "start"],
            ]
        );
    }

    #[test]
    fn systemd_uninstall_commands_only_stop_and_disable_before_file_removal() {
        assert_eq!(
            uninstall_commands(InitKind::Systemd),
            vec![vec![
                "systemctl",
                "disable",
                "--now",
                "miniboard-ipd.service"
            ]]
        );
        assert_eq!(
            post_uninstall_commands(InitKind::Systemd),
            vec![vec!["systemctl", "daemon-reload"]]
        );
    }

    #[test]
    fn sysv_update_rc_d_uninstall_stops_service_and_removes_boot_start() {
        assert_eq!(
            uninstall_commands(InitKind::SysVUpdateRcD),
            vec![
                vec!["service", "miniboard-ipd", "stop"],
                vec!["update-rc.d", "miniboard-ipd", "remove"],
            ]
        );
    }

    #[test]
    fn sysv_chkconfig_uninstall_stops_service_and_removes_boot_start() {
        assert_eq!(
            uninstall_commands(InitKind::SysVChkconfig),
            vec![
                vec!["service", "miniboard-ipd", "stop"],
                vec!["chkconfig", "--del", "miniboard-ipd"],
            ]
        );
    }

    #[test]
    fn apply_systemd_install_copies_binary_writes_unit_and_starts_service() {
        let mut ops = RecordingOps::default();
        apply_install(&request(InitKind::Systemd), &mut ops).unwrap();

        assert_eq!(
            ops.events[0],
            "copy:/tmp/miniboard-ipd:/usr/local/bin/miniboard-ipd:true"
        );
        assert_eq!(
            ops.events[1],
            "write:/etc/systemd/system/miniboard-ipd.service:false:true"
        );
        assert!(ops
            .events
            .contains(&"run:systemctl daemon-reload".to_string()));
        assert!(ops
            .events
            .contains(&"run:systemctl enable --now miniboard-ipd.service".to_string()));
    }

    #[test]
    fn apply_openwrt_install_writes_executable_init_script() {
        let mut ops = RecordingOps::default();
        apply_install(&request(InitKind::OpenWrtProcd), &mut ops).unwrap();

        assert!(ops
            .events
            .iter()
            .any(|event| event == "write:/etc/init.d/miniboard-ipd:true:true"));
        assert!(ops
            .events
            .iter()
            .any(|event| event == "run:/etc/init.d/miniboard-ipd enable"));
    }

    #[test]
    fn uninstall_stops_service_removes_script_and_binary() {
        let mut ops = RecordingOps::default();
        apply_uninstall(InitKind::Systemd, "/usr/local/bin/miniboard-ipd", &mut ops).unwrap();

        assert_eq!(
            ops.events,
            vec![
                "run:systemctl disable --now miniboard-ipd.service",
                "remove:/etc/systemd/system/miniboard-ipd.service",
                "remove:/usr/local/bin/miniboard-ipd",
                "run:systemctl daemon-reload",
            ]
        );
    }

    #[test]
    fn status_runs_init_specific_command() {
        let mut ops = RecordingOps::default();
        run_status(InitKind::Systemd, &mut ops).unwrap();

        assert_eq!(
            ops.events,
            vec!["run:systemctl status --no-pager miniboard-ipd.service"]
        );
    }
}
