use std::fmt;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Run(RunOptions),
    Install(RunOptions),
    Uninstall,
    Status,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOptions {
    pub interface: Option<String>,
    pub dhcp_fail_delay: Duration,
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
            "expected command: run, install, uninstall, status".to_string(),
        ));
    };

    match command.as_str() {
        "run" => parse_run_options(args).map(Command::Run),
        "install" => parse_run_options(args).map(Command::Install),
        "uninstall" => reject_extra("uninstall", args).map(|_| Command::Uninstall),
        "status" => reject_extra("status", args).map(|_| Command::Status),
        other => Err(CliError(format!("unknown command {other}"))),
    }
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
        out
    }
}

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
    };

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--interface" => {
                let value = args
                    .next()
                    .ok_or_else(|| CliError("--interface requires a value".to_string()))?;
                if !is_valid_linux_interface_name(&value) {
                    return Err(CliError(format!(
                        "invalid interface name {value:?}; must be a non-empty Linux device name under 16 bytes without whitespace, '/' or ':'"
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
        && !value
            .chars()
            .any(|ch| ch.is_whitespace() || ch == '/' || ch == ':')
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
            })
        );
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
}
