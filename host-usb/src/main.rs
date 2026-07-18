#[cfg(target_os = "linux")]
fn main() {
    if let Err(err) = real_main() {
        if err.kind() == std::io::ErrorKind::InvalidInput {
            eprintln!("{err}");
            std::process::exit(2);
        } else {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}

#[cfg(target_os = "linux")]
const INSTALLED_BINARY: &str = "/usr/local/bin/miniboard-ipd";

#[cfg(target_os = "linux")]
fn real_main() -> std::io::Result<()> {
    use miniboard_ipd::cli::{parse_args, Command};
    use miniboard_ipd::service_install::{
        apply_install, apply_uninstall, detect_init, run_status, InstallRequest, InstallSpec,
        RealInstallOps,
    };

    let command = parse_args(std::env::args().skip(1))
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err.to_string()))?;

    match command {
        Command::Run(options) => {
            miniboard_ipd::logging::info(&format!("starting foreground daemon: {:?}", options));
            miniboard_ipd::logging::info(&format!(
                "usb event mode: {:?}",
                miniboard_ipd::usb_events::choose_event_mode()
            ));
            miniboard_ipd::runtime::run_forever(options)?;
        }
        Command::Install(options) => {
            let request = InstallRequest {
                source_binary_path: std::env::current_exe()?,
                spec: InstallSpec {
                    binary_path: INSTALLED_BINARY.to_string(),
                    service_args: options.service_args(),
                },
                init: detect_init(&current_probe()),
            };
            let mut ops = RealInstallOps;
            apply_install(&request, &mut ops)?;
        }
        Command::Uninstall => {
            let mut ops = RealInstallOps;
            apply_uninstall(detect_init(&current_probe()), INSTALLED_BINARY, &mut ops)?;
        }
        Command::Status => {
            let mut ops = RealInstallOps;
            run_status(detect_init(&current_probe()), &mut ops)?;
        }
        Command::Version => {
            println!("{}", miniboard_ipd::cli::version_string());
        }
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn current_probe() -> miniboard_ipd::service_install::InitProbe {
    miniboard_ipd::service_install::InitProbe {
        has_openwrt_release: std::path::Path::new("/etc/openwrt_release").exists(),
        has_procd: std::path::Path::new("/sbin/procd").exists(),
        has_systemd_runtime: std::path::Path::new("/run/systemd/system").exists(),
        has_systemctl: command_exists("systemctl"),
        has_openrc_runtime: std::path::Path::new("/run/openrc/softlevel").exists(),
        has_rc_service: command_exists("rc-service"),
        has_update_rc_d: command_exists("update-rc.d"),
        has_chkconfig: command_exists("chkconfig"),
        pid1_comm: std::fs::read_to_string("/proc/1/comm")
            .ok()
            .map(|value| value.trim().to_string()),
        has_busybox: std::path::Path::new("/bin/busybox").exists(),
    }
}

#[cfg(target_os = "linux")]
fn command_exists(name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|path| path.join(name).exists()))
        .unwrap_or(false)
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!(
        "{}",
        miniboard_ipd::platform::unsupported_platform_message()
    );
    std::process::exit(1);
}
