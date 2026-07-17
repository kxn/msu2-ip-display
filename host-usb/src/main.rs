#[cfg(target_os = "linux")]
fn main() {
    use miniboard_ipd::cli::{parse_args, Command};

    match parse_args(std::env::args().skip(1)) {
        Ok(Command::Run(options)) => {
            miniboard_ipd::logging::info(&format!("run requested: {:?}", options));
        }
        Ok(Command::Install(options)) => {
            miniboard_ipd::logging::info(&format!("install requested: {:?}", options));
        }
        Ok(Command::Uninstall) => miniboard_ipd::logging::info("uninstall requested"),
        Ok(Command::Status) => miniboard_ipd::logging::info("status requested"),
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(2);
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("{}", miniboard_ipd::platform::unsupported_platform_message());
    std::process::exit(1);
}
