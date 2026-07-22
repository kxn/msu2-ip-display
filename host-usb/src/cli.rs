use std::fmt;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Run(RunOptions),
    Install(RunOptions),
    Uninstall,
    Status,
    Version,
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOptions {
    pub interface: Option<String>,
    pub dhcp_fail_delay: Duration,
    pub debug: bool,
    pub show: DisplayMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisplayMode {
    Text,
    Qr { template: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError(pub String);

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

pub fn parse_args<I, S>(args: I) -> Result<Command, CliError>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut args = args.into_iter().map(Into::into);
    let Some(command) = args.next() else {
        return Err(CliError(
            "expected command: run, install, uninstall, status, version".to_string(),
        ));
    };
    let remaining: Vec<String> = args.collect();

    match command.as_str() {
        "run" if is_help_request(&remaining) => Ok(Command::Help),
        "run" => parse_run_options(remaining.into_iter()).map(Command::Run),
        "install" if is_help_request(&remaining) => Ok(Command::Help),
        "install" => parse_run_options(remaining.into_iter()).map(Command::Install),
        "uninstall" => reject_extra("uninstall", remaining.into_iter()).map(|_| Command::Uninstall),
        "status" => reject_extra("status", remaining.into_iter()).map(|_| Command::Status),
        "version" | "--version" => {
            reject_extra(&command, remaining.into_iter()).map(|_| Command::Version)
        }
        "help" | "--help" | "-h" => {
            reject_extra(&command, remaining.into_iter()).map(|_| Command::Help)
        }
        other => Err(CliError(format!("unknown command {other}"))),
    }
}

pub fn version_string() -> String {
    format!("miniboard-ipd {}", env!("CARGO_PKG_VERSION"))
}

fn is_help_request(args: &[String]) -> bool {
    matches!(args, [arg] if arg == "--help" || arg == "-h")
}

pub fn help_text() -> &'static str {
    "miniboard-ipd - display this Linux host IPv4 address on an MSU2 MINI USB screen

Usage:
  miniboard-ipd run [options]
  miniboard-ipd install [options]
  miniboard-ipd uninstall
  miniboard-ipd status
  miniboard-ipd version
  miniboard-ipd --help

Commands:
  run        Run the daemon in the foreground.
  install    Install service files and enable boot start. Does not start the service.
  uninstall  Stop and remove the installed service and binary.
  status     Show service status.
  version    Show the installed version.

Options for run/install:
  --interface <name>                 Use a specific Linux network interface.
  --dhcp-fail-delay-seconds <secs>   Delay before showing DHCP failure. Default: 45.
  --show ip                          Display the host IP as text. Default.
  --show qr                          Display the host IP as a QR code using http://{ip}/.
  --show qr:<template>               Display a QR code using a template containing {ip}.
  --debug                            Enable debug logging.
"
}

impl RunOptions {
    pub fn service_args(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(interface) = &self.interface {
            out.push("--interface".to_string());
            out.push(interface.clone());
        }
        out.push("--dhcp-fail-delay-seconds".to_string());
        out.push(self.dhcp_fail_delay.as_secs().to_string());
        if self.debug {
            out.push("--debug".to_string());
        }
        match &self.show {
            DisplayMode::Text => {}
            DisplayMode::Qr { template } if template == DEFAULT_QR_TEMPLATE => {
                out.push("--show".to_string());
                out.push("qr".to_string());
            }
            DisplayMode::Qr { template } => {
                out.push("--show".to_string());
                out.push(format!("qr:{template}"));
            }
        }
        out
    }
}

pub const DEFAULT_QR_TEMPLATE: &str = "http://{ip}/";

fn reject_extra<I>(command: &str, mut args: I) -> Result<(), CliError>
where
    I: Iterator<Item = String>,
{
    if let Some(extra) = args.next() {
        return Err(CliError(format!(
            "{command} does not accept argument {extra}"
        )));
    }
    Ok(())
}

fn parse_run_options<I>(mut args: I) -> Result<RunOptions, CliError>
where
    I: Iterator<Item = String>,
{
    let mut options = RunOptions {
        interface: None,
        dhcp_fail_delay: Duration::from_secs(45),
        debug: false,
        show: DisplayMode::Text,
    };

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--interface" => {
                let value = args
                    .next()
                    .ok_or_else(|| CliError("--interface requires a value".to_string()))?;
                if !is_valid_linux_interface_name(&value) {
                    return Err(CliError(format!(
                        "invalid interface name {value:?}; must be a non-empty Linux device name under 16 bytes using only A-Z, a-z, 0-9, '_', '-', '.', or '@'"
                    )));
                }
                options.interface = Some(value);
            }
            "--dhcp-fail-delay-seconds" => {
                let value = args.next().ok_or_else(|| {
                    CliError("--dhcp-fail-delay-seconds requires a value".to_string())
                })?;
                let seconds: u64 = value.parse().map_err(|_| {
                    CliError("--dhcp-fail-delay-seconds requires an integer".to_string())
                })?;
                if seconds == 0 {
                    return Err(CliError(
                        "--dhcp-fail-delay-seconds must be > 0".to_string(),
                    ));
                }
                options.dhcp_fail_delay = Duration::from_secs(seconds);
            }
            "--debug" => {
                options.debug = true;
            }
            "--show" => {
                let value = args
                    .next()
                    .ok_or_else(|| CliError("--show requires a value".to_string()))?;
                options.show = parse_display_mode(&value)?;
            }
            other => return Err(CliError(format!("unknown option {other}"))),
        }
    }

    Ok(options)
}

fn is_valid_linux_interface_name(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value.len() < 16
        && value.is_ascii()
        && value.bytes().all(|byte| {
            matches!(
                byte,
                b'A'..=b'Z'
                    | b'a'..=b'z'
                    | b'0'..=b'9'
                    | b'_'
                    | b'-'
                    | b'.'
                    | b'@'
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_uses_defaults_without_config_file() {
        let command = parse_args(["run"]).unwrap();
        assert_eq!(
            command,
            Command::Run(RunOptions {
                interface: None,
                dhcp_fail_delay: Duration::from_secs(45),
                debug: false,
                show: DisplayMode::Text,
            })
        );
    }

    #[test]
    fn show_defaults_to_text_mode() {
        let command = parse_args(["run"]).unwrap();
        let Command::Run(options) = command else {
            panic!("expected run command");
        };

        assert_eq!(options.show, DisplayMode::Text);
    }

    #[test]
    fn show_qr_uses_default_template() {
        let command = parse_args(["run", "--show", "qr"]).unwrap();
        let Command::Run(options) = command else {
            panic!("expected run command");
        };

        assert_eq!(
            options.show,
            DisplayMode::Qr {
                template: "http://{ip}/".to_string()
            }
        );
    }

    #[test]
    fn show_qr_template_is_embedded_in_service_args() {
        let command = parse_args(["install", "--show", "qr:http://{ip}:8080/"]).unwrap();
        let Command::Install(options) = command else {
            panic!("expected install command");
        };

        assert_eq!(
            options.service_args(),
            [
                "--dhcp-fail-delay-seconds",
                "45",
                "--show",
                "qr:http://{ip}:8080/",
            ]
        );
    }

    #[test]
    fn invalid_show_values_are_rejected() {
        for value in ["text", "qr:", "qr:http://device/", "QR", "ip:http://{ip}/"] {
            let err = parse_args(["run", "--show", value]).unwrap_err();
            assert!(
                err.to_string().contains("--show"),
                "unexpected error for {value:?}: {err}"
            );
        }
    }

    #[test]
    fn show_qr_rejects_template_that_is_too_long_for_screen() {
        let value = format!("qr:http://{{ip}}:8080/{}", "a".repeat(80));
        let err = parse_args(["run", "--show", &value]).unwrap_err();
        assert!(err.to_string().contains("too long"));
    }

    #[test]
    fn install_preserves_interface_and_delay_for_service_command() {
        let command = parse_args([
            "install",
            "--interface",
            "eth0",
            "--dhcp-fail-delay-seconds",
            "90",
        ])
        .unwrap();
        let Command::Install(options) = command else {
            panic!("expected install command");
        };

        assert_eq!(options.interface.as_deref(), Some("eth0"));
        assert_eq!(options.dhcp_fail_delay, Duration::from_secs(90));
        assert!(!options.debug);
        assert_eq!(options.show, DisplayMode::Text);
        assert_eq!(
            options.service_args(),
            ["--interface", "eth0", "--dhcp-fail-delay-seconds", "90",]
        );
    }

    #[test]
    fn unknown_option_is_rejected() {
        let err = parse_args(["run", "--config", "/etc/miniboard-ipd.conf"]).unwrap_err();
        assert!(err.to_string().contains("unknown option --config"));
    }

    #[test]
    fn debug_flag_is_embedded_in_service_args() {
        let command = parse_args(["install", "--interface", "eth0", "--debug"]).unwrap();
        let Command::Install(options) = command else {
            panic!("expected install command");
        };

        assert!(options.debug);
        assert_eq!(
            options.service_args(),
            [
                "--interface",
                "eth0",
                "--dhcp-fail-delay-seconds",
                "45",
                "--debug",
            ]
        );
    }

    #[test]
    fn uninstall_and_status_parse_without_options() {
        assert_eq!(parse_args(["uninstall"]).unwrap(), Command::Uninstall);
        assert_eq!(parse_args(["status"]).unwrap(), Command::Status);
    }

    #[test]
    fn install_rejects_invalid_interface_names_for_service_embedding() {
        for value in [
            "",
            "eth 0",
            "eth0/1",
            "eth0:1",
            ".",
            "..",
            "1234567890123456",
            "foo;bar",
            "foo\"bar",
            "foo`bar",
            "$(id)",
            "foo&bar",
            "foo|bar",
            "foo'bar",
        ] {
            let err = parse_args(["install", "--interface", value]).unwrap_err();
            assert!(
                err.to_string().contains("invalid interface name"),
                "unexpected error for {value:?}: {err}"
            );
        }
    }

    #[test]
    fn install_accepts_linux_device_style_interface_names() {
        for value in ["eth0", "enp1s0", "wlan0", "br-lan", "eth0.10", "usb0@if2"] {
            let command = parse_args(["install", "--interface", value]).unwrap();
            let Command::Install(options) = command else {
                panic!("expected install command");
            };
            assert_eq!(options.interface.as_deref(), Some(value));
        }
    }

    #[test]
    fn version_commands_parse_without_options() {
        assert_eq!(parse_args(["--version"]).unwrap(), Command::Version);
        assert_eq!(parse_args(["version"]).unwrap(), Command::Version);
        assert_eq!(
            version_string(),
            format!("miniboard-ipd {}", env!("CARGO_PKG_VERSION"))
        );
    }

    #[test]
    fn help_commands_parse_without_options() {
        assert!(parse_args(["--help"]).is_ok());
        assert!(parse_args(["-h"]).is_ok());
        assert!(parse_args(["help"]).is_ok());
        assert!(parse_args(["run", "--help"]).is_ok());
        assert!(parse_args(["install", "-h"]).is_ok());
    }

    #[test]
    fn help_text_lists_commands_and_common_options() {
        let help = help_text();

        assert!(help.contains("Usage:"));
        assert!(help.contains("miniboard-ipd run [options]"));
        assert!(help.contains("run"));
        assert!(help.contains("install"));
        assert!(help.contains("--interface"));
        assert!(help.contains("--show"));
    }
}

fn parse_display_mode(value: &str) -> Result<DisplayMode, CliError> {
    match value {
        "ip" => Ok(DisplayMode::Text),
        "qr" => Ok(DisplayMode::Qr {
            template: DEFAULT_QR_TEMPLATE.to_string(),
        }),
        _ if value.starts_with("qr:") => {
            let template = &value[3..];
            if template.is_empty() {
                return Err(CliError(
                    "--show qr:<template> requires a non-empty template".to_string(),
                ));
            }
            if !template.contains("{ip}") {
                return Err(CliError(
                    "--show qr:<template> must contain {ip}".to_string(),
                ));
            }
            crate::qr_display::validate_template(template)
                .map_err(|err| CliError(format!("--show {err}")))?;
            Ok(DisplayMode::Qr {
                template: template.to_string(),
            })
        }
        _ => Err(CliError(
            "--show must be ip, qr, or qr:<template>".to_string(),
        )),
    }
}
