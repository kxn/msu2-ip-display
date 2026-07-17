pub mod cli;
pub mod display;
pub mod logging;
pub mod protocol;
pub mod platform;

#[cfg(test)]
mod platform_tests {
    #[test]
    #[cfg(not(target_os = "linux"))]
    fn non_linux_platform_has_unsupported_message() {
        assert!(crate::platform::unsupported_platform_message()
            .contains("miniboard-ipd is only supported on Linux"));
    }
}
