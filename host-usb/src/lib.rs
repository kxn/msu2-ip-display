pub mod cli;
pub mod daemon;
pub mod device_scan;
pub mod display;
pub mod ip_detect;
pub mod logging;
pub mod platform;
pub mod protocol;
pub mod qr_display;
pub mod runtime;
pub mod serial;
pub mod service_install;
pub mod usb_events;

#[cfg(test)]
mod platform_tests {
    #[test]
    #[cfg(not(target_os = "linux"))]
    fn non_linux_platform_has_unsupported_message() {
        assert!(crate::platform::unsupported_platform_message()
            .contains("miniboard-ipd is only supported on Linux"));
    }
}
