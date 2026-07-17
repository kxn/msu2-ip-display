# Host USB IP Display Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `host-usb/miniboard-ipd`, a low-dependency Linux daemon that detects an MSU2 MINI USB screen and displays the host IPv4 address.

**Architecture:** Create an independent Rust crate under `host-usb/`. Keep Linux I/O at the edges behind small traits, with pure tested modules for CLI parsing, protocol packet generation, IP selection, display layout, daemon state transitions, and service script generation.

**Tech Stack:** Rust 2021, standard library, `libc` for Linux termios/netlink/poll/syscalls, MSU2 serial protocol, Linux sysfs, Linux rtnetlink, systemd/OpenRC/OpenWrt procd/SysV/BusyBox init scripts.

## Global Constraints

- Linux-only host program.
- v1 displays IPv4 only.
- v1 does not use a config file; install-time command-line options are embedded into the service/init script.
- Required serial path is `921600 8N1 + RTS/CTS`.
- Required handshake is `00 4D 53 4E 43 4E`, i.e. `\0MSNCN`.
- Required USB VID/PID is `1A86:FE0C`.
- Required screen pages: pending `3826`, DHCP failed `3726`, IP background `3926`, official digits at `4026 + digit`.
- Default DHCP failure delay is `45s`.
- Required release targets are `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl`; ARMv7 is optional after target hardware is known.
- Avoid mandatory `libudev`, Python, Tauri, GUI dependencies, and a large async runtime.
- Follow TDD for production code: write failing tests first, verify RED, implement minimal code, verify GREEN, commit.

---

## File Structure

Create these files:

- `host-usb/Cargo.toml`: Rust package metadata and minimal dependencies.
- `host-usb/src/lib.rs`: module exports.
- `host-usb/src/main.rs`: CLI entry point.
- `host-usb/src/cli.rs`: command-line parsing and install-time argument preservation.
- `host-usb/src/protocol.rs`: MSU2 wire packet helpers.
- `host-usb/src/display.rs`: screen state commands and IP glyph layout.
- `host-usb/src/ip_detect.rs`: pure IPv4 selection rules plus Linux collector boundary types.
- `host-usb/src/daemon.rs`: top-level state machine over injected device/IP/screen traits.
- `host-usb/src/device_scan.rs`: Linux sysfs tty VID/PID matching.
- `host-usb/src/usb_events.rs`: Linux netlink uevent listener with polling fallback signal.
- `host-usb/src/serial.rs`: Linux termios serial open/read/write/error classification.
- `host-usb/src/service_install.rs`: init detection and service script generation/application.
- `host-usb/src/logging.rs`: simple log helpers.
- `host-usb/README.md`: usage, install examples, and hardware assumptions.

Do not share code with the Tauri flasher in v1. Duplicate the small packet helpers in `host-usb/src/protocol.rs` so the daemon can remain independent and Linux-only.

---

### Task 1: Rust Crate, CLI, And Options

**Files:**
- Create: `host-usb/Cargo.toml`
- Create: `host-usb/src/lib.rs`
- Create: `host-usb/src/main.rs`
- Create: `host-usb/src/cli.rs`
- Create: `host-usb/src/logging.rs`
- Modify: `host-usb/README.md`

**Interfaces:**
- Produces: `cli::Command`
- Produces: `cli::RunOptions`
- Produces: `cli::parse_args<I, S>(args: I) -> Result<Command, CliError>`
- Produces: `RunOptions::service_args(&self) -> Vec<String>`
- Produces: `logging::{info, warn, debug}`

- [ ] **Step 1: Create crate files**

Create `host-usb/Cargo.toml`:

```toml
[package]
name = "miniboard-ipd"
version = "0.1.0"
edition = "2021"
description = "Linux daemon that displays host IPv4 on an MSU2 MINI USB screen"

[dependencies]
libc = "0.2"
```

Create `host-usb/src/lib.rs`:

```rust
pub mod cli;
pub mod logging;
```

Create `host-usb/src/main.rs`:

```rust
use miniboard_ipd::cli::{parse_args, Command};

fn main() {
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
```

Create `host-usb/src/logging.rs`:

```rust
pub fn info(message: &str) {
    eprintln!("INFO miniboard-ipd: {message}");
}

pub fn warn(message: &str) {
    eprintln!("WARN miniboard-ipd: {message}");
}

pub fn debug(message: &str) {
    if std::env::var_os("MINIBOARD_IPD_DEBUG").is_some() {
        eprintln!("DEBUG miniboard-ipd: {message}");
    }
}
```

- [ ] **Step 2: Write failing CLI tests**

Create `host-usb/src/cli.rs` with the tests first and minimal type declarations that do not pass:

```rust
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

pub fn parse_args<I, S>(_args: I) -> Result<Command, CliError>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    Err(CliError("not implemented".to_string()))
}

impl RunOptions {
    pub fn service_args(&self) -> Vec<String> {
        Vec::new()
    }
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
            [
                "--interface",
                "eth0",
                "--dhcp-fail-delay-seconds",
                "90",
            ]
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
}
```

- [ ] **Step 3: Run tests and confirm RED**

Run:

```powershell
cd host-usb
cargo test cli::tests -- --nocapture
```

Expected: tests fail because `parse_args(["run"])` returns `CliError("not implemented")`.

- [ ] **Step 4: Implement CLI parser**

Replace `parse_args` and `RunOptions::service_args` in `host-usb/src/cli.rs` with:

```rust
pub fn parse_args<I, S>(args: I) -> Result<Command, CliError>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut args = args.into_iter().map(Into::into);
    let Some(command) = args.next() else {
        return Err(CliError("expected command: run, install, uninstall, status".to_string()));
    };

    match command.as_str() {
        "run" => parse_run_options(args).map(Command::Run),
        "install" => parse_run_options(args).map(Command::Install),
        "uninstall" => reject_extra("uninstall", args).map(|_| Command::Uninstall),
        "status" => reject_extra("status", args).map(|_| Command::Status),
        other => Err(CliError(format!("unknown command {other}"))),
    }
}

fn reject_extra<I>(command: &str, mut args: I) -> Result<(), CliError>
where
    I: Iterator<Item = String>,
{
    if let Some(extra) = args.next() {
        return Err(CliError(format!("{command} does not accept argument {extra}")));
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
                if value.is_empty() {
                    return Err(CliError("--interface requires a non-empty value".to_string()));
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
                    return Err(CliError("--dhcp-fail-delay-seconds must be > 0".to_string()));
                }
                options.dhcp_fail_delay = Duration::from_secs(seconds);
            }
            other => return Err(CliError(format!("unknown option {other}"))),
        }
    }

    Ok(options)
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
```

- [ ] **Step 5: Run tests and confirm GREEN**

Run:

```powershell
cd host-usb
cargo test cli::tests -- --nocapture
```

Expected: all `cli::tests` pass.

- [ ] **Step 6: Update README**

Replace `host-usb/README.md` with:

````markdown
# Host USB

`miniboard-ipd` is the Linux host-side daemon for showing a headless device's IPv4 address on an MSU2 MINI USB screen.

Design:

- `docs/superpowers/specs/2026-07-17-host-usb-ip-display-design.md`

Examples:

```bash
miniboard-ipd run
miniboard-ipd run --interface eth0
miniboard-ipd install --interface eth0 --dhcp-fail-delay-seconds 45
```

v1 has no config file. Options passed to `install` are embedded into the installed service/init script.
````

- [ ] **Step 7: Commit**

Run:

```powershell
git add host-usb
git commit -m "feat: scaffold host usb daemon cli"
```

Expected: commit contains the Rust crate scaffold, CLI tests, and README update.

---

### Task 2: MSU2 Protocol Packet Helpers

**Files:**
- Modify: `host-usb/src/lib.rs`
- Create: `host-usb/src/protocol.rs`

**Interfaces:**
- Produces: `protocol::HANDSHAKE: [u8; 6]`
- Produces: `protocol::set_xy_packet(x: u16, y: u16) -> [u8; 6]`
- Produces: `protocol::set_size_packet(width: u16, height: u16) -> [u8; 6]`
- Produces: `protocol::set_color_packet(foreground: u16, background: u16) -> [u8; 6]`
- Produces: `protocol::show_photo_packet(page: u16) -> [u8; 6]`
- Produces: `protocol::ram_init_packet(fill: u8) -> [u8; 6]`
- Produces: `protocol::add_ram_masked_packet(address: u32) -> [u8; 6]`
- Produces: `protocol::load_ram_mix_show_packet(background_page: u16) -> [u8; 6]`
- Produces: `protocol::load_lcd_address_packet() -> [u8; 6]`
- Produces: `protocol::write_lcd_data_packet(size: u16, data: &[u8; 256]) -> [u8; 390]`
- Produces: constants `SCREEN_WIDTH`, `SCREEN_HEIGHT`, `DHCP_FAILED_PAGE`, `PENDING_PAGE`, `IP_BACKGROUND_PAGE`, `DIGIT_RESOURCE_PAGE`

- [ ] **Step 1: Export protocol module**

Modify `host-usb/src/lib.rs`:

```rust
pub mod cli;
pub mod logging;
pub mod protocol;
```

- [ ] **Step 2: Write failing protocol tests**

Create `host-usb/src/protocol.rs` with:

```rust
pub const HANDSHAKE: [u8; 6] = [0x00, b'M', b'S', b'N', b'C', b'N'];
pub const SCREEN_WIDTH: u16 = 160;
pub const SCREEN_HEIGHT: u16 = 80;
pub const DHCP_FAILED_PAGE: u16 = 3726;
pub const PENDING_PAGE: u16 = 3826;
pub const IP_BACKGROUND_PAGE: u16 = 3926;
pub const DIGIT_RESOURCE_PAGE: u16 = 4026;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_packets_match_verified_protocol() {
        assert_eq!(HANDSHAKE, [0x00, 0x4d, 0x53, 0x4e, 0x43, 0x4e]);
        assert_eq!(set_xy_packet(0, 0), [0x02, 0x00, 0x00, 0x00, 0x00, 0x00]);
        assert_eq!(set_size_packet(160, 80), [0x02, 0x01, 0x00, 0xa0, 0x00, 0x50]);
        assert_eq!(show_photo_packet(3926), [0x02, 0x03, 0x00, 0x0f, 0x56, 0x00]);
    }

    #[test]
    fn official_digit_ram_packets_match_hardware_probe() {
        assert_eq!(ram_init_packet(0), [0x02, 0x03, 0x0d, 0x00, 0x00, 0x00]);
        assert_eq!(
            add_ram_masked_packet((4026 + 5) * 256),
            [0x02, 0x03, 0x0f, 0x0f, 0xbb, 0x00]
        );
        assert_eq!(
            load_ram_mix_show_packet(3926),
            [0x02, 0x03, 0x11, 0x0f, 0x56, 0x00]
        );
    }

    #[test]
    fn lcd_direct_write_packet_matches_flasher() {
        assert_eq!(load_lcd_address_packet(), [0x02, 0x03, 0x07, 0x00, 0x00, 0x00]);
        let data = [0x5a; 256];
        let packet = write_lcd_data_packet(16, &data);
        assert_eq!(packet.len(), 390);
        assert_eq!(&packet[0..6], &[0x04, 0x00, 0x5a, 0x5a, 0x5a, 0x5a]);
        assert_eq!(&packet[384..390], &[0x02, 0x03, 0x08, 0x00, 0x10, 0x00]);
    }
}
```

- [ ] **Step 3: Run tests and confirm RED**

Run:

```powershell
cd host-usb
cargo test protocol::tests -- --nocapture
```

Expected: compile failure because packet helper functions are missing.

- [ ] **Step 4: Implement packet helpers**

Add this implementation above the test module in `host-usb/src/protocol.rs`:

```rust
#[inline]
fn hi16(value: u16) -> u8 {
    ((value >> 8) & 0xff) as u8
}

#[inline]
fn lo16(value: u16) -> u8 {
    (value & 0xff) as u8
}

#[inline]
fn addr_hi(value: u32) -> u8 {
    ((value >> 16) & 0xff) as u8
}

#[inline]
fn addr_mid(value: u32) -> u8 {
    ((value >> 8) & 0xff) as u8
}

#[inline]
fn addr_lo(value: u32) -> u8 {
    (value & 0xff) as u8
}

pub fn set_xy_packet(x: u16, y: u16) -> [u8; 6] {
    [0x02, 0x00, hi16(x), lo16(x), hi16(y), lo16(y)]
}

pub fn set_size_packet(width: u16, height: u16) -> [u8; 6] {
    [0x02, 0x01, hi16(width), lo16(width), hi16(height), lo16(height)]
}

pub fn set_color_packet(foreground: u16, background: u16) -> [u8; 6] {
    [
        0x02,
        0x02,
        hi16(foreground),
        lo16(foreground),
        hi16(background),
        lo16(background),
    ]
}

pub fn show_photo_packet(page: u16) -> [u8; 6] {
    [0x02, 0x03, 0x00, hi16(page), lo16(page), 0x00]
}

pub fn ram_init_packet(fill: u8) -> [u8; 6] {
    [0x02, 0x03, 0x0d, fill, 0x00, 0x00]
}

pub fn add_ram_masked_packet(address: u32) -> [u8; 6] {
    [
        0x02,
        0x03,
        0x0f,
        addr_hi(address),
        addr_mid(address),
        addr_lo(address),
    ]
}

pub fn load_ram_mix_show_packet(background_page: u16) -> [u8; 6] {
    let address = background_page as u32 * 256;
    [
        0x02,
        0x03,
        0x11,
        addr_hi(address),
        addr_mid(address),
        addr_lo(address),
    ]
}

pub fn load_lcd_address_packet() -> [u8; 6] {
    [0x02, 0x03, 0x07, 0x00, 0x00, 0x00]
}

pub fn write_lcd_data_packet(size: u16, data: &[u8; 256]) -> [u8; 390] {
    let mut packet = [0u8; 390];

    for index in 0..64usize {
        let src = index * 4;
        let dst = index * 6;
        packet[dst] = 0x04;
        packet[dst + 1] = index as u8;
        packet[dst + 2] = data[src];
        packet[dst + 3] = data[src + 1];
        packet[dst + 4] = data[src + 2];
        packet[dst + 5] = data[src + 3];
    }

    packet[384] = 0x02;
    packet[385] = 0x03;
    packet[386] = 0x08;
    packet[387] = hi16(size);
    packet[388] = lo16(size);
    packet[389] = 0x00;
    packet
}
```

- [ ] **Step 5: Run tests and confirm GREEN**

Run:

```powershell
cd host-usb
cargo test protocol::tests -- --nocapture
```

Expected: all protocol tests pass.

- [ ] **Step 6: Commit**

Run:

```powershell
git add host-usb/src/lib.rs host-usb/src/protocol.rs
git commit -m "feat: add msu2 protocol packets"
```

Expected: commit contains protocol constants, packet helpers, and tests.

---

### Task 3: Display Renderer And IP Layout

**Files:**
- Modify: `host-usb/src/lib.rs`
- Create: `host-usb/src/display.rs`

**Interfaces:**
- Produces: `display::WireWrite`
- Produces: `display::IpLayout`
- Produces: `display::DisplayRenderer`
- Produces: `DisplayRenderer::pending() -> Vec<WireWrite>`
- Produces: `DisplayRenderer::dhcp_failed() -> Vec<WireWrite>`
- Produces: `DisplayRenderer::ip(ip: std::net::Ipv4Addr) -> Vec<WireWrite>`

- [ ] **Step 1: Export display module**

Modify `host-usb/src/lib.rs`:

```rust
pub mod cli;
pub mod display;
pub mod logging;
pub mod protocol;
```

- [ ] **Step 2: Write failing display tests**

Create `host-usb/src/display.rs`:

```rust
use std::net::Ipv4Addr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireWrite {
    pub bytes: Vec<u8>,
    pub wait_for_echo: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DigitGlyph {
    pub x: u16,
    pub y: u16,
    pub digit: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DotGlyph {
    pub x: u16,
    pub y: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpLayout {
    pub digits: Vec<DigitGlyph>,
    pub dots: Vec<DotGlyph>,
}

pub struct DisplayRenderer;

impl DisplayRenderer {
    pub fn pending() -> Vec<WireWrite> {
        Vec::new()
    }

    pub fn dhcp_failed() -> Vec<WireWrite> {
        Vec::new()
    }

    pub fn ip(_ip: Ipv4Addr) -> Vec<WireWrite> {
        Vec::new()
    }

    pub fn layout_ip(_ip: Ipv4Addr) -> IpLayout {
        IpLayout {
            digits: Vec::new(),
            dots: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{
        show_photo_packet, DHCP_FAILED_PAGE, IP_BACKGROUND_PAGE, PENDING_PAGE,
    };

    #[test]
    fn page_state_commands_show_expected_pages() {
        assert_eq!(
            DisplayRenderer::pending()[0].bytes,
            show_photo_packet(PENDING_PAGE).to_vec()
        );
        assert_eq!(
            DisplayRenderer::dhcp_failed()[0].bytes,
            show_photo_packet(DHCP_FAILED_PAGE).to_vec()
        );
    }

    #[test]
    fn max_ip_layout_is_centered_in_two_rows() {
        let layout = DisplayRenderer::layout_ip(Ipv4Addr::new(255, 255, 255, 255));
        assert_eq!(layout.digits.len(), 12);
        assert_eq!(layout.dots.len(), 2);
        assert_eq!(layout.digits[0], DigitGlyph { x: 4, y: 3, digit: 2 });
        assert_eq!(layout.digits[5], DigitGlyph { x: 132, y: 3, digit: 5 });
        assert_eq!(layout.digits[6], DigitGlyph { x: 4, y: 44, digit: 2 });
        assert_eq!(layout.dots[0], DotGlyph { x: 77, y: 28 });
        assert_eq!(layout.dots[1], DotGlyph { x: 77, y: 69 });
    }

    #[test]
    fn short_ip_rows_are_independently_centered() {
        let layout = DisplayRenderer::layout_ip(Ipv4Addr::new(10, 0, 1, 5));
        assert_eq!(layout.digits[0].x, 52);
        assert_eq!(layout.digits[2].x, 108);
        assert_eq!(layout.digits[3].x, 52);
        assert_eq!(layout.digits[4].x, 84);
    }

    #[test]
    fn ip_render_starts_with_background_and_loads_ram_mix() {
        let writes = DisplayRenderer::ip(Ipv4Addr::new(192, 168, 1, 204));
        assert_eq!(writes[0].bytes, show_photo_packet(IP_BACKGROUND_PAGE).to_vec());
        assert!(writes.iter().any(|write| write.bytes == [0x02, 0x03, 0x0d, 0x00, 0x00, 0x00]));
        assert!(writes.iter().any(|write| write.bytes == [0x02, 0x03, 0x11, 0x0f, 0x56, 0x00]));
    }
}
```

- [ ] **Step 3: Run tests and confirm RED**

Run:

```powershell
cd host-usb
cargo test display::tests -- --nocapture
```

Expected: tests fail because renderer returns empty commands and empty layout.

- [ ] **Step 4: Implement display renderer**

Replace the `DisplayRenderer` implementation with:

```rust
use crate::protocol::{
    add_ram_masked_packet, load_lcd_address_packet, load_ram_mix_show_packet, ram_init_packet,
    set_color_packet, set_size_packet, set_xy_packet, show_photo_packet, write_lcd_data_packet,
    DHCP_FAILED_PAGE, DIGIT_RESOURCE_PAGE, IP_BACKGROUND_PAGE, PENDING_PAGE,
};

const DIGIT_WIDTH: u16 = 24;
const DIGIT_HEIGHT: u16 = 33;
const DOT_SLOT_WIDTH: u16 = 8;
const DOT_SIZE: u16 = 5;
const ROW_GAP: u16 = 8;
const SCREEN_WIDTH: u16 = 160;
const SCREEN_HEIGHT: u16 = 80;
const RGB565_GREEN: u16 = 0x07e0;

impl DisplayRenderer {
    pub fn pending() -> Vec<WireWrite> {
        vec![WireWrite {
            bytes: show_photo_packet(PENDING_PAGE).to_vec(),
            wait_for_echo: false,
        }]
    }

    pub fn dhcp_failed() -> Vec<WireWrite> {
        vec![WireWrite {
            bytes: show_photo_packet(DHCP_FAILED_PAGE).to_vec(),
            wait_for_echo: false,
        }]
    }

    pub fn ip(ip: Ipv4Addr) -> Vec<WireWrite> {
        let mut writes = vec![
            packet(show_photo_packet(IP_BACKGROUND_PAGE), false),
            packet(ram_init_packet(0), false),
        ];

        let layout = Self::layout_ip(ip);
        for glyph in layout.digits {
            let address = (DIGIT_RESOURCE_PAGE as u32 + glyph.digit as u32) * 256;
            writes.push(packet(set_xy_packet(glyph.x, glyph.y), false));
            writes.push(packet(set_size_packet(DIGIT_WIDTH, DIGIT_HEIGHT), false));
            writes.push(packet(add_ram_masked_packet(address), false));
        }

        writes.push(packet(set_color_packet(RGB565_GREEN, 0), false));
        writes.push(packet(load_ram_mix_show_packet(IP_BACKGROUND_PAGE), false));

        for dot in layout.dots {
            writes.extend(dot_writes(dot));
        }

        writes
    }

    pub fn layout_ip(ip: Ipv4Addr) -> IpLayout {
        let octets = ip.octets();
        let rows = [
            format!("{}.{}", octets[0], octets[1]),
            format!("{}.{}", octets[2], octets[3]),
        ];
        let total_height = DIGIT_HEIGHT * 2 + ROW_GAP;
        let start_y = (SCREEN_HEIGHT - total_height) / 2;
        let mut digits = Vec::new();
        let mut dots = Vec::new();

        for (row_index, row) in rows.iter().enumerate() {
            let y = start_y + row_index as u16 * (DIGIT_HEIGHT + ROW_GAP);
            let mut x = (SCREEN_WIDTH - row_width(row)) / 2;
            for ch in row.chars() {
                if ch == '.' {
                    dots.push(DotGlyph {
                        x: x + (DOT_SLOT_WIDTH - DOT_SIZE) / 2,
                        y: y + DIGIT_HEIGHT - DOT_SIZE - 3,
                    });
                    x += DOT_SLOT_WIDTH;
                } else {
                    let digit = ch.to_digit(10).expect("IPv4 rows contain only digits and dot");
                    digits.push(DigitGlyph {
                        x,
                        y,
                        digit: digit as u8,
                    });
                    x += DIGIT_WIDTH;
                }
            }
        }

        IpLayout { digits, dots }
    }
}

fn packet(bytes: [u8; 6], wait_for_echo: bool) -> WireWrite {
    WireWrite {
        bytes: bytes.to_vec(),
        wait_for_echo,
    }
}

fn row_width(row: &str) -> u16 {
    row.chars()
        .map(|ch| if ch == '.' { DOT_SLOT_WIDTH } else { DIGIT_WIDTH })
        .sum()
}

fn dot_writes(dot: DotGlyph) -> Vec<WireWrite> {
    let mut dot_bytes = [0u8; 256];
    for pixel in dot_bytes
        .chunks_exact_mut(2)
        .take((DOT_SIZE as usize) * (DOT_SIZE as usize))
    {
        pixel.copy_from_slice(&RGB565_GREEN.to_be_bytes());
    }
    vec![
        packet(set_xy_packet(dot.x, dot.y), false),
        packet(set_size_packet(DOT_SIZE, DOT_SIZE), false),
        packet(load_lcd_address_packet(), true),
        WireWrite {
            bytes: write_lcd_data_packet(DOT_SIZE * DOT_SIZE * 2, &dot_bytes).to_vec(),
            wait_for_echo: false,
        },
    ]
}
```

- [ ] **Step 5: Run tests and confirm GREEN**

Run:

```powershell
cd host-usb
cargo test display::tests -- --nocapture
```

Expected: all display tests pass.

- [ ] **Step 6: Commit**

Run:

```powershell
git add host-usb/src/lib.rs host-usb/src/display.rs
git commit -m "feat: render msu2 ip display commands"
```

Expected: commit contains display renderer and layout tests.

---

### Task 4: Pure IPv4 Selection Rules

**Files:**
- Modify: `host-usb/src/lib.rs`
- Create: `host-usb/src/ip_detect.rs`

**Interfaces:**
- Produces: `ip_detect::NetworkSnapshot`
- Produces: `ip_detect::AddressCandidate`
- Produces: `ip_detect::Route`
- Produces: `ip_detect::SelectionConfig`
- Produces: `ip_detect::Selection`
- Produces: `ip_detect::select_ipv4(snapshot: &NetworkSnapshot, config: &SelectionConfig) -> Selection`

- [ ] **Step 1: Export IP module**

Modify `host-usb/src/lib.rs`:

```rust
pub mod cli;
pub mod display;
pub mod ip_detect;
pub mod logging;
pub mod protocol;
```

- [ ] **Step 2: Write failing IP selection tests**

Create `host-usb/src/ip_detect.rs`:

```rust
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

pub fn select_ipv4(_snapshot: &NetworkSnapshot, _config: &SelectionConfig) -> Selection {
    Selection::Pending
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
}
```

- [ ] **Step 3: Run tests and confirm RED**

Run:

```powershell
cd host-usb
cargo test ip_detect::tests -- --nocapture
```

Expected: tests fail because `select_ipv4` always returns `Pending`.

- [ ] **Step 4: Implement pure selection**

Replace `select_ipv4` with:

```rust
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
                if candidates.iter().any(|candidate| candidate.interface == *interface && is_link_local(candidate.address)) {
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
            if candidates.iter().any(|candidate| is_link_local(candidate.address)) {
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
```

- [ ] **Step 5: Run tests and confirm GREEN**

Run:

```powershell
cd host-usb
cargo test ip_detect::tests -- --nocapture
```

Expected: all IP selection tests pass.

- [ ] **Step 6: Commit**

Run:

```powershell
git add host-usb/src/lib.rs host-usb/src/ip_detect.rs
git commit -m "feat: select displayable ipv4"
```

Expected: commit contains pure IPv4 selection logic and tests.

---

### Task 5: Daemon State Machine

**Files:**
- Modify: `host-usb/src/lib.rs`
- Create: `host-usb/src/daemon.rs`

**Interfaces:**
- Consumes: `cli::RunOptions`
- Consumes: `display::DisplayRenderer`
- Consumes: `ip_detect::{Selection, select_ipv4}`
- Produces: `daemon::DaemonState`
- Produces: `daemon::DaemonEvent`
- Produces: `daemon::Daemon`
- Produces: `Daemon::handle_event(&mut self, event: DaemonEvent) -> Vec<DaemonAction>`

- [ ] **Step 1: Export daemon module**

Modify `host-usb/src/lib.rs`:

```rust
pub mod cli;
pub mod daemon;
pub mod display;
pub mod ip_detect;
pub mod logging;
pub mod protocol;
```

- [ ] **Step 2: Write failing daemon tests**

Create `host-usb/src/daemon.rs`:

```rust
use std::net::Ipv4Addr;
use std::time::{Duration, Instant};

use crate::cli::RunOptions;
use crate::ip_detect::{NetworkSnapshot, SelectionConfig};

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
    NetworkSnapshot { snapshot: NetworkSnapshot, now: Instant },
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
    _options: RunOptions,
}

impl Daemon {
    pub fn new(options: RunOptions) -> Self {
        Self {
            state: DaemonState::Listening,
            _options: options,
        }
    }

    pub fn handle_event(&mut self, _event: DaemonEvent) -> Vec<DaemonAction> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ip_detect::{AddressCandidate, Route};

    fn options() -> RunOptions {
        RunOptions {
            interface: None,
            dhcp_fail_delay: Duration::from_secs(45),
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
            routes: vec![Route { interface: "eth0".to_string(), is_default: true }],
        }
    }

    #[test]
    fn connect_success_shows_pending_page() {
        let mut daemon = Daemon::new(options());
        assert_eq!(daemon.handle_event(DaemonEvent::DeviceCandidateFound), vec![DaemonAction::OpenDevice]);
        assert_eq!(daemon.state, DaemonState::Connecting);

        assert_eq!(daemon.handle_event(DaemonEvent::HandshakeOk), vec![DaemonAction::ShowPending]);
        assert_eq!(daemon.state, DaemonState::ConnectedPendingIp);
    }

    #[test]
    fn displayable_ip_switches_to_showing_ip() {
        let mut daemon = Daemon::new(options());
        daemon.handle_event(DaemonEvent::DeviceCandidateFound);
        daemon.handle_event(DaemonEvent::HandshakeOk);
        let now = Instant::now();

        assert_eq!(
            daemon.handle_event(DaemonEvent::NetworkSnapshot { snapshot: snapshot([192, 168, 1, 20]), now }),
            vec![DaemonAction::ShowIp(Ipv4Addr::new(192, 168, 1, 20))]
        );
        assert_eq!(daemon.state, DaemonState::ConnectedShowingIp(Ipv4Addr::new(192, 168, 1, 20)));
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
            daemon.handle_event(DaemonEvent::NetworkSnapshot { snapshot: link_local.clone(), now: start }),
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

        assert_eq!(daemon.handle_event(DaemonEvent::DeviceDisconnected), vec![DaemonAction::CloseDevice]);
        assert_eq!(daemon.state, DaemonState::Listening);
    }
}
```

- [ ] **Step 3: Run tests and confirm RED**

Run:

```powershell
cd host-usb
cargo test daemon::tests -- --nocapture
```

Expected: tests fail because `handle_event` returns no actions.

- [ ] **Step 4: Implement state transitions**

Replace `Daemon` with:

```rust
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
                    crate::ip_detect::Selection::Show(ip) => {
                        self.failure_since = None;
                        if self.state == DaemonState::ConnectedShowingIp(ip) {
                            Vec::new()
                        } else {
                            self.state = DaemonState::ConnectedShowingIp(ip);
                            vec![DaemonAction::ShowIp(ip)]
                        }
                    }
                    crate::ip_detect::Selection::Pending => {
                        self.failure_since = None;
                        if self.state == DaemonState::ConnectedPendingIp {
                            Vec::new()
                        } else {
                            self.state = DaemonState::ConnectedPendingIp;
                            vec![DaemonAction::ShowPending]
                        }
                    }
                    crate::ip_detect::Selection::FailureCandidate => {
                        let since = *self.failure_since.get_or_insert(now);
                        if now.duration_since(since) >= self.options.dhcp_fail_delay {
                            if self.state == DaemonState::ConnectedDhcpFailed {
                                Vec::new()
                            } else {
                                self.state = DaemonState::ConnectedDhcpFailed;
                                vec![DaemonAction::ShowDhcpFailed]
                            }
                        } else {
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
```

- [ ] **Step 5: Run tests and confirm GREEN**

Run:

```powershell
cd host-usb
cargo test daemon::tests -- --nocapture
```

Expected: all daemon tests pass.

- [ ] **Step 6: Commit**

Run:

```powershell
git add host-usb/src/lib.rs host-usb/src/daemon.rs
git commit -m "feat: add host daemon state machine"
```

Expected: commit contains pure daemon state machine and tests.

---

### Task 6: Linux Device Scan And Disconnect Classification

**Files:**
- Modify: `host-usb/src/lib.rs`
- Create: `host-usb/src/device_scan.rs`
- Create: `host-usb/src/serial.rs`

**Interfaces:**
- Produces: `device_scan::TtyDevice { path: PathBuf, name: String }`
- Produces: `device_scan::match_target_tty(name: &str, ancestry: &[UsbAttrs]) -> bool`
- Produces: `serial::SerialErrorKind`
- Produces: `serial::classify_errno(errno: i32) -> SerialErrorKind`
- Produces: `serial::classify_poll_revents(revents: i16) -> Option<SerialErrorKind>`

- [ ] **Step 1: Export modules**

Modify `host-usb/src/lib.rs`:

```rust
pub mod cli;
pub mod daemon;
pub mod device_scan;
pub mod display;
pub mod ip_detect;
pub mod logging;
pub mod protocol;
pub mod serial;
```

- [ ] **Step 2: Write failing scan and serial tests**

Create `host-usb/src/device_scan.rs`:

```rust
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TtyDevice {
    pub path: PathBuf,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsbAttrs {
    pub id_vendor: String,
    pub id_product: String,
}

pub fn match_target_tty(_name: &str, _ancestry: &[UsbAttrs]) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_verified_vid_pid_in_usb_ancestry() {
        assert!(match_target_tty(
            "ttyACM0",
            &[UsbAttrs { id_vendor: "1a86".to_string(), id_product: "fe0c".to_string() }]
        ));
    }

    #[test]
    fn rejects_wrong_vid_pid() {
        assert!(!match_target_tty(
            "ttyUSB0",
            &[UsbAttrs { id_vendor: "1a86".to_string(), id_product: "7523".to_string() }]
        ));
    }

    #[test]
    fn accepts_uppercase_sysfs_hex() {
        assert!(match_target_tty(
            "ttyACM0",
            &[UsbAttrs { id_vendor: "1A86".to_string(), id_product: "FE0C".to_string() }]
        ));
    }
}
```

Create `host-usb/src/serial.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerialErrorKind {
    Disconnected,
    Timeout,
    Other,
}

pub fn classify_errno(_errno: i32) -> SerialErrorKind {
    SerialErrorKind::Other
}

pub fn classify_poll_revents(_revents: i16) -> Option<SerialErrorKind> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_gone_errors_are_disconnected() {
        assert_eq!(classify_errno(libc::EIO), SerialErrorKind::Disconnected);
        assert_eq!(classify_errno(libc::ENODEV), SerialErrorKind::Disconnected);
        assert_eq!(classify_errno(libc::ENXIO), SerialErrorKind::Disconnected);
    }

    #[test]
    fn poll_hangup_and_error_are_disconnected() {
        assert_eq!(classify_poll_revents(libc::POLLHUP), Some(SerialErrorKind::Disconnected));
        assert_eq!(classify_poll_revents(libc::POLLERR), Some(SerialErrorKind::Disconnected));
        assert_eq!(classify_poll_revents(libc::POLLNVAL), Some(SerialErrorKind::Disconnected));
    }
}
```

- [ ] **Step 3: Run tests and confirm RED**

Run:

```powershell
cd host-usb
cargo test device_scan::tests serial::tests -- --nocapture
```

Expected: tests fail because target matching returns false and serial classifiers return `Other`/`None`.

- [ ] **Step 4: Implement pure scan matching and serial classification**

Replace `match_target_tty`:

```rust
pub fn match_target_tty(_name: &str, ancestry: &[UsbAttrs]) -> bool {
    ancestry.iter().any(|attrs| {
        attrs.id_vendor.eq_ignore_ascii_case("1a86")
            && attrs.id_product.eq_ignore_ascii_case("fe0c")
    })
}
```

Replace serial classifiers:

```rust
pub fn classify_errno(errno: i32) -> SerialErrorKind {
    match errno {
        libc::EIO | libc::ENODEV | libc::ENXIO => SerialErrorKind::Disconnected,
        libc::EAGAIN => SerialErrorKind::Timeout,
        _ => SerialErrorKind::Other,
    }
}

pub fn classify_poll_revents(revents: i16) -> Option<SerialErrorKind> {
    let disconnected = libc::POLLHUP | libc::POLLERR | libc::POLLNVAL;
    if revents & disconnected != 0 {
        Some(SerialErrorKind::Disconnected)
    } else {
        None
    }
}
```

- [ ] **Step 5: Add Linux-only implementation stubs behind cfg**

Append to `host-usb/src/device_scan.rs`:

```rust
#[cfg(target_os = "linux")]
pub fn scan_target_ttys() -> std::io::Result<Vec<TtyDevice>> {
    scan_target_ttys_from(std::path::Path::new("/sys/class/tty"), std::path::Path::new("/dev"))
}

#[cfg(target_os = "linux")]
pub fn scan_target_ttys_from(
    sys_class_tty: &std::path::Path,
    dev_root: &std::path::Path,
) -> std::io::Result<Vec<TtyDevice>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(sys_class_tty)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        let device_path = entry.path().join("device");
        let mut attrs = Vec::new();
        collect_usb_attrs(&device_path, &mut attrs);
        if match_target_tty(&name, &attrs) {
            out.push(TtyDevice {
                path: dev_root.join(&name),
                name,
            });
        }
    }
    Ok(out)
}

#[cfg(target_os = "linux")]
fn collect_usb_attrs(path: &std::path::Path, attrs: &mut Vec<UsbAttrs>) {
    let mut current = path.to_path_buf();
    for _ in 0..8 {
        let vendor = std::fs::read_to_string(current.join("idVendor"));
        let product = std::fs::read_to_string(current.join("idProduct"));
        if let (Ok(vendor), Ok(product)) = (vendor, product) {
            attrs.push(UsbAttrs {
                id_vendor: vendor.trim().to_string(),
                id_product: product.trim().to_string(),
            });
        }
        if !current.pop() {
            break;
        }
    }
}
```

- [ ] **Step 6: Run tests and confirm GREEN**

Run:

```powershell
cd host-usb
cargo test device_scan::tests serial::tests -- --nocapture
```

Expected: all device scan and serial classification tests pass.

- [ ] **Step 7: Commit**

Run:

```powershell
git add host-usb/src/lib.rs host-usb/src/device_scan.rs host-usb/src/serial.rs
git commit -m "feat: classify linux usb screen devices"
```

Expected: commit contains pure matching/classification plus Linux sysfs scan entry points.

---

### Task 7: Service Installation Script Generation

**Files:**
- Modify: `host-usb/src/lib.rs`
- Create: `host-usb/src/service_install.rs`

**Interfaces:**
- Produces: `service_install::InitKind`
- Produces: `service_install::InstallSpec`
- Produces: `service_install::detect_init(probe: &InitProbe) -> InitKind`
- Produces: `service_install::render_service(kind: InitKind, spec: &InstallSpec) -> ServiceRender`

- [ ] **Step 1: Export service_install module**

Modify `host-usb/src/lib.rs`:

```rust
pub mod cli;
pub mod daemon;
pub mod device_scan;
pub mod display;
pub mod ip_detect;
pub mod logging;
pub mod protocol;
pub mod serial;
pub mod service_install;
```

- [ ] **Step 2: Write failing service generation tests**

Create `host-usb/src/service_install.rs`:

```rust
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

pub fn detect_init(_probe: &InitProbe) -> InitKind {
    InitKind::Unknown
}

pub fn render_service(_kind: InitKind, _spec: &InstallSpec) -> ServiceRender {
    ServiceRender { path: String::new(), contents: String::new() }
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
        assert!(render.contents.contains("ExecStart=/usr/local/bin/miniboard-ipd run --interface eth0 --dhcp-fail-delay-seconds 45"));
        assert!(render.contents.contains("WantedBy=multi-user.target"));
    }

    #[test]
    fn openwrt_script_uses_procd_respawn() {
        let render = render_service(InitKind::OpenWrtProcd, &spec());
        assert_eq!(render.path, "/etc/init.d/miniboard-ipd");
        assert!(render.contents.contains("USE_PROCD=1"));
        assert!(render.contents.contains("procd_set_param command /usr/local/bin/miniboard-ipd run --interface eth0 --dhcp-fail-delay-seconds 45"));
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
```

- [ ] **Step 3: Run tests and confirm RED**

Run:

```powershell
cd host-usb
cargo test service_install::tests -- --nocapture
```

Expected: tests fail because detection returns `Unknown` and generated services are empty.

- [ ] **Step 4: Implement init detection and service rendering**

Replace `detect_init` and `render_service`:

```rust
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
        contents: format!("#!/bin/sh\ncase \"$1\" in\n  start) {} & ;;\n  stop) pkill -f \"{} run\" || true ;;\nesac\n", command_line(spec), spec.binary_path),
    }
}

fn render_template(path: &str, spec: &InstallSpec) -> ServiceRender {
    ServiceRender {
        path: path.to_string(),
        contents: format!("#!/bin/sh\nexec {}\n", command_line(spec)),
    }
}
```

- [ ] **Step 5: Run tests and confirm GREEN**

Run:

```powershell
cd host-usb
cargo test service_install::tests -- --nocapture
```

Expected: all service install tests pass.

- [ ] **Step 6: Commit**

Run:

```powershell
git add host-usb/src/lib.rs host-usb/src/service_install.rs
git commit -m "feat: generate linux service scripts"
```

Expected: commit contains init detection and script generation.

---

### Task 8: Linux Serial Session And USB Event Foundations

**Files:**
- Modify: `host-usb/src/main.rs`
- Modify: `host-usb/src/serial.rs`
- Create: `host-usb/src/usb_events.rs`

**Interfaces:**
- Consumes: protocol and serial classification.
- Produces: `serial::SerialSession`
- Produces: `usb_events::EventMode`

- [ ] **Step 1: Write failing serial timeout counter test**

Add to `host-usb/src/serial.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeoutCounter {
    consecutive: u8,
    threshold: u8,
}

impl TimeoutCounter {
    pub fn new(threshold: u8) -> Self {
        Self { consecutive: 0, threshold }
    }

    pub fn record_success(&mut self) {
        self.consecutive = 0;
    }

    pub fn record_timeout(&mut self) -> bool {
        self.consecutive += 1;
        self.consecutive >= self.threshold
    }
}

#[cfg(test)]
mod timeout_counter_tests {
    use super::*;

    #[test]
    fn disconnects_after_three_consecutive_timeouts() {
        let mut counter = TimeoutCounter::new(3);
        assert!(!counter.record_timeout());
        assert!(!counter.record_timeout());
        assert!(counter.record_timeout());
        counter.record_success();
        assert!(!counter.record_timeout());
    }
}
```

- [ ] **Step 2: Run test and confirm GREEN**

Run:

```powershell
cd host-usb
cargo test serial::timeout_counter_tests -- --nocapture
```

Expected: timeout counter test passes. This small helper is already fully implemented because it is isolated and documents the disconnect threshold.

- [ ] **Step 3: Add Linux serial session skeleton**

Append to `host-usb/src/serial.rs`:

```rust
#[cfg(target_os = "linux")]
pub struct SerialSession {
    fd: std::os::fd::OwnedFd,
}

#[cfg(target_os = "linux")]
impl SerialSession {
    pub fn open(path: &std::path::Path) -> std::io::Result<Self> {
        use std::ffi::CString;
        use std::os::fd::FromRawFd;
        use std::os::unix::ffi::OsStrExt;

        let path = CString::new(path.as_os_str().as_bytes()).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "path contains nul byte")
        })?;
        let fd = unsafe {
            libc::open(
                path.as_ptr(),
                libc::O_RDWR | libc::O_NOCTTY | libc::O_NONBLOCK,
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let fd = unsafe { std::os::fd::OwnedFd::from_raw_fd(fd) };
        configure_921600_rtscts(fd.as_raw_fd())?;
        Ok(Self { fd })
    }

    pub fn write_all(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        let mut offset = 0;
        while offset < bytes.len() {
            let rc = unsafe {
                libc::write(
                    self.fd.as_raw_fd(),
                    bytes[offset..].as_ptr().cast(),
                    bytes.len() - offset,
                )
            };
            if rc < 0 {
                return Err(std::io::Error::last_os_error());
            }
            offset += rc as usize;
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn configure_921600_rtscts(fd: std::os::fd::RawFd) -> std::io::Result<()> {
    let mut termios = std::mem::MaybeUninit::<libc::termios>::uninit();
    if unsafe { libc::tcgetattr(fd, termios.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    let mut termios = unsafe { termios.assume_init() };
    unsafe { libc::cfmakeraw(&mut termios) };
    termios.c_cflag |= libc::CLOCAL | libc::CREAD | libc::CRTSCTS;
    termios.c_cflag &= !libc::CSIZE;
    termios.c_cflag |= libc::CS8;
    termios.c_cflag &= !libc::PARENB;
    termios.c_cflag &= !libc::CSTOPB;
    if unsafe { libc::cfsetspeed(&mut termios, libc::B921600) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &termios) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}
```

Add imports near the top of `host-usb/src/serial.rs`:

```rust
#[cfg(target_os = "linux")]
use std::os::fd::{AsRawFd, RawFd};
```

- [ ] **Step 4: Add uevent module with poll fallback mode**

Create `host-usb/src/usb_events.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventMode {
    Netlink,
    Polling,
}

#[cfg(target_os = "linux")]
pub fn choose_event_mode() -> EventMode {
    if netlink_socket_available() {
        EventMode::Netlink
    } else {
        EventMode::Polling
    }
}

#[cfg(target_os = "linux")]
fn netlink_socket_available() -> bool {
    let fd = unsafe { libc::socket(libc::AF_NETLINK, libc::SOCK_DGRAM, libc::NETLINK_KOBJECT_UEVENT) };
    if fd < 0 {
        return false;
    }
    unsafe { libc::close(fd) };
    true
}

#[cfg(not(target_os = "linux"))]
pub fn choose_event_mode() -> EventMode {
    EventMode::Polling
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_mode_values_are_stable_for_logging() {
        assert_eq!(format!("{:?}", EventMode::Netlink), "Netlink");
        assert_eq!(format!("{:?}", EventMode::Polling), "Polling");
    }
}
```

Modify `host-usb/src/lib.rs` to export `usb_events`:

```rust
pub mod usb_events;
```

- [ ] **Step 5: Wire main run path**

Replace `host-usb/src/main.rs` with a thin dispatch:

```rust
use miniboard_ipd::cli::{parse_args, Command};

fn main() {
    match parse_args(std::env::args().skip(1)) {
        Ok(Command::Run(options)) => {
            miniboard_ipd::logging::info(&format!("starting foreground daemon: {:?}", options));
            miniboard_ipd::logging::info("runtime loop is added in Task 10");
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
```

This keeps the binary buildable while the final runtime loop is added in Task 10 after service install application exists.

- [ ] **Step 6: Run tests and Linux compile check**

Run:

```powershell
cd host-usb
cargo test -- --nocapture
cargo check
```

Expected: all tests pass and crate checks on the development machine. On non-Linux hosts, Linux-only serial functions are cfg-gated.

- [ ] **Step 7: Commit**

Run:

```powershell
git add host-usb/src
git commit -m "feat: add linux serial foundations"
```

Expected: commit contains serial foundations, timeout counter, event mode, and buildable main.

---

### Task 9: Install, Uninstall, Status, And Documentation

**Files:**
- Modify: `host-usb/src/main.rs`
- Modify: `host-usb/src/service_install.rs`
- Modify: `host-usb/README.md`
- Modify: `README.md`
- Modify: `docs/README.md`

**Interfaces:**
- Consumes CLI options and service renderer.
- Produces `service_install::InstallRequest`
- Produces `service_install::InstallOps`
- Produces `service_install::RealInstallOps`
- Produces `service_install::{apply_install, apply_uninstall, run_status}`
- Produces functional `install`, `uninstall`, and `status` commands.
- Produces documentation for build/test/use.

- [ ] **Step 1: Add service application tests**

Add to `host-usb/src/service_install.rs`:

```rust
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallRequest {
    pub source_binary_path: PathBuf,
    pub spec: InstallSpec,
    pub init: InitKind,
}

pub trait InstallOps {
    fn copy_file(&mut self, from: &std::path::Path, to: &str, executable: bool) -> std::io::Result<()>;
    fn write_file(&mut self, path: &str, contents: &str, executable: bool) -> std::io::Result<()>;
    fn remove_file(&mut self, path: &str) -> std::io::Result<()>;
    fn run(&mut self, command: &[String]) -> std::io::Result<()>;
}

pub fn install_commands(kind: InitKind) -> Vec<Vec<String>> {
    match kind {
        InitKind::Systemd => vec![
            vec!["systemctl".into(), "daemon-reload".into()],
            vec!["systemctl".into(), "enable".into(), "--now".into(), "miniboard-ipd.service".into()],
        ],
        InitKind::OpenRc => vec![
            vec!["rc-update".into(), "add".into(), "miniboard-ipd".into(), "default".into()],
            vec!["rc-service".into(), "miniboard-ipd".into(), "start".into()],
        ],
        InitKind::OpenWrtProcd => vec![
            vec!["/etc/init.d/miniboard-ipd".into(), "enable".into()],
            vec!["/etc/init.d/miniboard-ipd".into(), "start".into()],
        ],
        InitKind::SysV => vec![vec!["service".into(), "miniboard-ipd".into(), "start".into()]],
        InitKind::BusyBox => vec![vec!["/etc/init.d/S99miniboard-ipd".into(), "start".into()]],
        _ => Vec::new(),
    }
}

pub fn uninstall_commands(kind: InitKind) -> Vec<Vec<String>> {
    match kind {
        InitKind::Systemd => vec![
            vec!["systemctl".into(), "disable".into(), "--now".into(), "miniboard-ipd.service".into()],
            vec!["systemctl".into(), "daemon-reload".into()],
        ],
        InitKind::OpenRc => vec![
            vec!["rc-service".into(), "miniboard-ipd".into(), "stop".into()],
            vec!["rc-update".into(), "del".into(), "miniboard-ipd".into(), "default".into()],
        ],
        InitKind::OpenWrtProcd => vec![
            vec!["/etc/init.d/miniboard-ipd".into(), "stop".into()],
            vec!["/etc/init.d/miniboard-ipd".into(), "disable".into()],
        ],
        InitKind::SysV => vec![vec!["service".into(), "miniboard-ipd".into(), "stop".into()]],
        InitKind::BusyBox => vec![vec!["/etc/init.d/S99miniboard-ipd".into(), "stop".into()]],
        _ => Vec::new(),
    }
}

pub fn status_command(kind: InitKind) -> Vec<String> {
    match kind {
        InitKind::Systemd => vec!["systemctl".into(), "status".into(), "--no-pager".into(), "miniboard-ipd.service".into()],
        InitKind::OpenRc => vec!["rc-service".into(), "miniboard-ipd".into(), "status".into()],
        InitKind::OpenWrtProcd => vec!["/etc/init.d/miniboard-ipd".into(), "status".into()],
        InitKind::SysV => vec!["service".into(), "miniboard-ipd".into(), "status".into()],
        InitKind::BusyBox => vec!["pgrep".into(), "-af".into(), "miniboard-ipd".into()],
        _ => vec!["pgrep".into(), "-af".into(), "miniboard-ipd".into()],
    }
}

pub fn apply_install(_request: &InstallRequest, _ops: &mut dyn InstallOps) -> std::io::Result<()> {
    Ok(())
}

pub fn apply_uninstall(_kind: InitKind, _binary_path: &str, _ops: &mut dyn InstallOps) -> std::io::Result<()> {
    Ok(())
}

pub fn run_status(_kind: InitKind, _ops: &mut dyn InstallOps) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod command_tests {
    use super::*;

    #[derive(Default)]
    struct RecordingOps {
        events: Vec<String>,
    }

    impl InstallOps for RecordingOps {
        fn copy_file(&mut self, from: &std::path::Path, to: &str, executable: bool) -> std::io::Result<()> {
            self.events.push(format!("copy:{}:{to}:{executable}", from.display()));
            Ok(())
        }

        fn write_file(&mut self, path: &str, contents: &str, executable: bool) -> std::io::Result<()> {
            self.events.push(format!("write:{path}:{executable}:{}", contents.contains("miniboard-ipd run")));
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
    fn apply_systemd_install_copies_binary_writes_unit_and_starts_service() {
        let mut ops = RecordingOps::default();
        apply_install(&request(InitKind::Systemd), &mut ops).unwrap();

        assert_eq!(ops.events[0], "copy:/tmp/miniboard-ipd:/usr/local/bin/miniboard-ipd:true");
        assert_eq!(ops.events[1], "write:/etc/systemd/system/miniboard-ipd.service:false:true");
        assert!(ops.events.contains(&"run:systemctl daemon-reload".to_string()));
        assert!(ops.events.contains(&"run:systemctl enable --now miniboard-ipd.service".to_string()));
    }

    #[test]
    fn apply_openwrt_install_writes_executable_init_script() {
        let mut ops = RecordingOps::default();
        apply_install(&request(InitKind::OpenWrtProcd), &mut ops).unwrap();

        assert!(ops.events.iter().any(|event| event == "write:/etc/init.d/miniboard-ipd:true:true"));
        assert!(ops.events.iter().any(|event| event == "run:/etc/init.d/miniboard-ipd enable"));
    }

    #[test]
    fn uninstall_stops_service_removes_script_and_binary() {
        let mut ops = RecordingOps::default();
        apply_uninstall(InitKind::Systemd, "/usr/local/bin/miniboard-ipd", &mut ops).unwrap();

        assert!(ops.events.contains(&"run:systemctl disable --now miniboard-ipd.service".to_string()));
        assert!(ops.events.contains(&"remove:/etc/systemd/system/miniboard-ipd.service".to_string()));
        assert!(ops.events.contains(&"remove:/usr/local/bin/miniboard-ipd".to_string()));
    }
}
```

- [ ] **Step 2: Run service tests**

Run:

```powershell
cd host-usb
cargo test service_install::command_tests -- --nocapture
```

Expected: service command tests pass.

- [ ] **Step 3: Implement service application**

Replace the stub implementations with:

```rust
pub fn apply_install(request: &InstallRequest, ops: &mut dyn InstallOps) -> std::io::Result<()> {
    let rendered = render_service(request.init, &request.spec);
    ops.copy_file(&request.source_binary_path, &request.spec.binary_path, true)?;
    ops.write_file(&rendered.path, &rendered.contents, service_needs_executable(request.init))?;
    for command in install_commands(request.init) {
        ops.run(&command)?;
    }
    Ok(())
}

pub fn apply_uninstall(kind: InitKind, binary_path: &str, ops: &mut dyn InstallOps) -> std::io::Result<()> {
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
    Ok(())
}

pub fn run_status(kind: InitKind, ops: &mut dyn InstallOps) -> std::io::Result<()> {
    ops.run(&status_command(kind))
}

fn service_needs_executable(kind: InitKind) -> bool {
    matches!(kind, InitKind::OpenRc | InitKind::OpenWrtProcd | InitKind::SysV | InitKind::BusyBox)
}
```

Add `RealInstallOps` in the same file:

```rust
pub struct RealInstallOps;

impl InstallOps for RealInstallOps {
    fn copy_file(&mut self, from: &std::path::Path, to: &str, executable: bool) -> std::io::Result<()> {
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
            Err(std::io::Error::new(std::io::ErrorKind::Other, format!("{program} exited with {status}")))
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
```

- [ ] **Step 4: Replace main with install/status dispatch**

Replace `host-usb/src/main.rs`:

```rust
use miniboard_ipd::cli::{parse_args, Command};
use miniboard_ipd::service_install::{
    apply_install, apply_uninstall, detect_init, run_status, InitProbe, InstallRequest, InstallSpec,
    RealInstallOps,
};

const INSTALLED_BINARY: &str = "/usr/local/bin/miniboard-ipd";

fn main() {
    if let Err(err) = real_main() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn real_main() -> Result<(), Box<dyn std::error::Error>> {
    match parse_args(std::env::args().skip(1)) {
        Ok(Command::Run(options)) => {
            miniboard_ipd::logging::info(&format!("starting foreground daemon: {:?}", options));
            miniboard_ipd::logging::info("runtime loop is wired in Task 10");
        }
        Ok(Command::Install(options)) => {
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
        Ok(Command::Uninstall) => {
            let mut ops = RealInstallOps;
            apply_uninstall(detect_init(&current_probe()), INSTALLED_BINARY, &mut ops)?;
        }
        Ok(Command::Status) => {
            let mut ops = RealInstallOps;
            run_status(detect_init(&current_probe()), &mut ops)?;
        }
        Err(err) => {
            return Err(Box::new(err));
        }
    }
    Ok(())
}

fn current_probe() -> InitProbe {
    InitProbe {
        has_openwrt_release: std::path::Path::new("/etc/openwrt_release").exists(),
        has_procd: std::path::Path::new("/sbin/procd").exists(),
        has_systemd_runtime: std::path::Path::new("/run/systemd/system").exists(),
        has_systemctl: command_exists("systemctl"),
        has_openrc_runtime: std::path::Path::new("/run/openrc/softlevel").exists(),
        has_rc_service: command_exists("rc-service"),
        has_update_rc_d: command_exists("update-rc.d"),
        has_chkconfig: command_exists("chkconfig"),
        pid1_comm: std::fs::read_to_string("/proc/1/comm").ok().map(|s| s.trim().to_string()),
        has_busybox: std::path::Path::new("/bin/busybox").exists(),
    }
}

fn command_exists(name: &str) -> bool {
    std::env::var_os("PATH")
        .and_then(|paths| {
            std::env::split_paths(&paths)
                .map(|path| path.join(name))
                .find(|path| path.exists())
        })
        .is_some()
}
```

- [ ] **Step 5: Update README docs**

Update `host-usb/README.md` to include:

````markdown
## Build

```bash
cargo test
cargo build --release
```

## Commands

```bash
miniboard-ipd run
miniboard-ipd run --interface eth0
miniboard-ipd install --interface eth0 --dhcp-fail-delay-seconds 45
miniboard-ipd uninstall
miniboard-ipd status
```

`install` stores command-line options in the generated service/init command. v1 does not read a config file.
````

Update root `README.md` under common commands with:

````markdown
```powershell
cd host-usb
cargo test
```
````

Update `docs/README.md` with a link to the implementation plan:

```markdown
- `superpowers/plans/2026-07-17-host-usb-ip-display.md`: host-side IP daemon implementation plan.
```

- [ ] **Step 6: Run verification**

Run:

```powershell
cd host-usb
cargo test -- --nocapture
cargo build --release
```

Expected:

- host-usb tests pass.
- host-usb release build succeeds.

- [ ] **Step 7: Commit**

Run:

```powershell
git add host-usb README.md docs/README.md
git commit -m "feat: install host usb daemon service"
```

Expected: commit contains real install/uninstall/status command dispatch, docs, and verified build/test state.

---

### Task 10: Runtime Loop, Linux Network Collection, And Hardware Validation

**Files:**
- Modify: `host-usb/src/lib.rs`
- Modify: `host-usb/src/main.rs`
- Modify: `host-usb/src/display.rs`
- Modify: `host-usb/src/ip_detect.rs`
- Modify: `host-usb/src/serial.rs`
- Create: `host-usb/src/runtime.rs`

**Interfaces:**
- Produces: `ip_detect::collect_network_snapshot() -> std::io::Result<NetworkSnapshot>`
- Produces: `display::DisplayRenderer::keepalive() -> Vec<WireWrite>`
- Produces: `serial::SerialSession::{handshake, send_writes}`
- Produces: `runtime::run_forever(options: RunOptions) -> std::io::Result<()>`
- Produces testable runtime loop over injected clock/device/network/screen traits.

- [ ] **Step 1: Export runtime module**

Modify `host-usb/src/lib.rs`:

```rust
pub mod runtime;
```

- [ ] **Step 2: Add runtime tests before implementation**

Create `host-usb/src/runtime.rs` with pure tests over fake dependencies:

```rust
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::cli::RunOptions;
use crate::daemon::{Daemon, DaemonAction, DaemonEvent};
use crate::device_scan::TtyDevice;
use crate::display::{DisplayRenderer, WireWrite};
use crate::ip_detect::NetworkSnapshot;

pub trait RuntimeIo {
    fn scan_devices(&mut self) -> std::io::Result<Vec<TtyDevice>>;
    fn connect(&mut self, device: &TtyDevice) -> std::io::Result<()>;
    fn disconnect(&mut self);
    fn network_snapshot(&mut self) -> std::io::Result<NetworkSnapshot>;
    fn send_writes(&mut self, writes: &[WireWrite]) -> std::io::Result<()>;
    fn sleep(&mut self, duration: Duration);
    fn now(&self) -> Instant;
}

pub struct Runtime<T> {
    daemon: Daemon,
    io: T,
    connected: bool,
    last_keepalive: Instant,
}

impl<T: RuntimeIo> Runtime<T> {
    pub fn new(options: RunOptions, io: T) -> Self {
        let now = io.now();
        Self {
            daemon: Daemon::new(options),
            io,
            connected: false,
            last_keepalive: now,
        }
    }

    pub fn tick(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub fn run_forever(_options: RunOptions) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ip_detect::{AddressCandidate, Route};

    #[derive(Default)]
    struct FakeIo {
        now: Option<Instant>,
        devices: Vec<TtyDevice>,
        snapshot: NetworkSnapshot,
        events: Vec<String>,
    }

    impl RuntimeIo for FakeIo {
        fn scan_devices(&mut self) -> std::io::Result<Vec<TtyDevice>> {
            Ok(self.devices.clone())
        }

        fn connect(&mut self, device: &TtyDevice) -> std::io::Result<()> {
            self.events.push(format!("connect:{}", device.path.display()));
            Ok(())
        }

        fn disconnect(&mut self) {
            self.events.push("disconnect".to_string());
        }

        fn network_snapshot(&mut self) -> std::io::Result<NetworkSnapshot> {
            Ok(self.snapshot.clone())
        }

        fn send_writes(&mut self, writes: &[WireWrite]) -> std::io::Result<()> {
            self.events.push(format!("writes:{}", writes.len()));
            Ok(())
        }

        fn sleep(&mut self, duration: Duration) {
            self.events.push(format!("sleep:{}", duration.as_millis()));
        }

        fn now(&self) -> Instant {
            self.now.unwrap_or_else(Instant::now)
        }
    }

    fn options() -> RunOptions {
        RunOptions { interface: None, dhcp_fail_delay: Duration::from_secs(45) }
    }

    #[test]
    fn device_connection_shows_pending_then_ip() {
        let start = Instant::now();
        let mut io = FakeIo { now: Some(start), ..FakeIo::default() };
        io.devices.push(TtyDevice { path: PathBuf::from("/dev/ttyACM0"), name: "ttyACM0".to_string() });
        io.snapshot = NetworkSnapshot {
            addresses: vec![AddressCandidate {
                interface: "eth0".to_string(),
                address: Ipv4Addr::new(192, 168, 1, 20),
                is_dynamic: true,
                is_up: true,
                is_lower_up: true,
            }],
            routes: vec![Route { interface: "eth0".to_string(), is_default: true }],
        };

        let mut runtime = Runtime::new(options(), io);
        runtime.tick().unwrap();

        assert!(runtime.io.events.iter().any(|event| event == "connect:/dev/ttyACM0"));
        assert!(runtime.io.events.iter().filter(|event| event.starts_with("writes:")).count() >= 2);
    }
}
```

- [ ] **Step 3: Run tests and confirm RED**

Run:

```powershell
cd host-usb
cargo test runtime::tests -- --nocapture
```

Expected: runtime test fails because `tick` does not scan, connect, or send writes.

- [ ] **Step 4: Implement runtime tick loop**

Replace `Runtime::tick` with:

```rust
pub fn tick(&mut self) -> std::io::Result<()> {
    if !self.connected {
        let devices = self.io.scan_devices()?;
        let Some(device) = devices.first() else {
            self.io.sleep(Duration::from_millis(500));
            return Ok(());
        };

        for action in self.daemon.handle_event(DaemonEvent::DeviceCandidateFound) {
            self.apply_action(action)?;
        }
        self.io.connect(device)?;
        self.connected = true;
        for action in self.daemon.handle_event(DaemonEvent::HandshakeOk) {
            self.apply_action(action)?;
        }
    }

    let snapshot = self.io.network_snapshot()?;
    let now = self.io.now();
    for action in self.daemon.handle_event(DaemonEvent::NetworkSnapshot { snapshot, now }) {
        self.apply_action(action)?;
    }

    if now.duration_since(self.last_keepalive) >= Duration::from_secs(10) {
        self.io.send_writes(&DisplayRenderer::keepalive())?;
        self.last_keepalive = now;
    }

    self.io.sleep(Duration::from_millis(500));
    Ok(())
}

fn apply_action(&mut self, action: DaemonAction) -> std::io::Result<()> {
    match action {
        DaemonAction::OpenDevice => Ok(()),
        DaemonAction::CloseDevice => {
            self.io.disconnect();
            self.connected = false;
            Ok(())
        }
        DaemonAction::ShowPending => self.io.send_writes(&DisplayRenderer::pending()),
        DaemonAction::ShowDhcpFailed => self.io.send_writes(&DisplayRenderer::dhcp_failed()),
        DaemonAction::ShowIp(ip) => self.io.send_writes(&DisplayRenderer::ip(ip)),
    }
}
```

On any `connect`, `send_writes`, or `network_snapshot` error classified as device disconnect, feed `DaemonEvent::DeviceDisconnected`, close the serial fd, and return to scanning. Treat one serial timeout as transient; the `TimeoutCounter` from Task 8 converts three consecutive timeouts into disconnect.

- [ ] **Step 5: Add Linux RuntimeIo implementation**

In `host-usb/src/runtime.rs`, add a Linux implementation:

```rust
#[cfg(target_os = "linux")]
pub struct LinuxRuntimeIo {
    session: Option<crate::serial::SerialSession>,
}

#[cfg(target_os = "linux")]
impl LinuxRuntimeIo {
    pub fn new() -> Self {
        Self { session: None }
    }
}

#[cfg(target_os = "linux")]
impl RuntimeIo for LinuxRuntimeIo {
    fn scan_devices(&mut self) -> std::io::Result<Vec<TtyDevice>> {
        crate::device_scan::scan_target_ttys()
    }

    fn connect(&mut self, device: &TtyDevice) -> std::io::Result<()> {
        let mut session = crate::serial::SerialSession::open(&device.path)?;
        session.handshake()?;
        self.session = Some(session);
        Ok(())
    }

    fn disconnect(&mut self) {
        self.session = None;
    }

    fn network_snapshot(&mut self) -> std::io::Result<NetworkSnapshot> {
        crate::ip_detect::collect_network_snapshot()
    }

    fn send_writes(&mut self, writes: &[WireWrite]) -> std::io::Result<()> {
        let Some(session) = self.session.as_mut() else {
            return Err(std::io::Error::new(std::io::ErrorKind::NotConnected, "screen not connected"));
        };
        session.send_writes(writes)
    }

    fn sleep(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }

    fn now(&self) -> Instant {
        Instant::now()
    }
}
```

Implement `run_forever` as an infinite loop around `Runtime::tick()` with disconnect recovery logging:

```rust
#[cfg(target_os = "linux")]
pub fn run_forever(options: RunOptions) -> std::io::Result<()> {
    let io = LinuxRuntimeIo::new();
    let mut runtime = Runtime::new(options, io);
    loop {
        if let Err(err) = runtime.tick() {
            crate::logging::warn(&format!("runtime tick failed: {err}"));
            runtime.connected = false;
            runtime.io.disconnect();
            runtime.io.sleep(Duration::from_millis(500));
        }
    }
}
```

Add a non-Linux `run_forever` that returns `Unsupported`.

- [ ] **Step 6: Add serial handshake and write helpers**

In `host-usb/src/serial.rs`, implement:

```rust
#[cfg(target_os = "linux")]
impl SerialSession {
    pub fn handshake(&mut self) -> std::io::Result<()> {
        self.write_all(&crate::protocol::HANDSHAKE)?;
        let reply = self.read_reply(Duration::from_millis(500))?;
        if contains_sequence(&reply, &crate::protocol::HANDSHAKE) {
            Ok(())
        } else {
            Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "handshake did not echo MSNCN"))
        }
    }

    pub fn send_writes(&mut self, writes: &[crate::display::WireWrite]) -> std::io::Result<()> {
        for write in writes {
            self.write_all(&write.bytes)?;
            if write.wait_for_echo {
                let _reply = self.read_reply(Duration::from_millis(500))?;
            }
        }
        Ok(())
    }
}
```

Reuse the flasher's `contains_sequence` behavior in `host-usb/src/protocol.rs` instead of depending on the flasher crate. Implement `read_reply` with `poll` on the tty fd; map `POLLHUP`, `POLLERR`, `POLLNVAL`, `EIO`, `ENODEV`, and `ENXIO` through the existing disconnect classifier.

- [ ] **Step 7: Add display keepalive**

Add to `host-usb/src/display.rs`:

```rust
impl DisplayRenderer {
    pub fn keepalive() -> Vec<WireWrite> {
        // One bottom-right pixel. The mock assets reserve this pixel as background.
        dot_writes(DotGlyph { x: 159, y: 79 })
    }
}
```

If hardware testing shows the bottom-right pixel is visible on any page, move the keepalive coordinate to a verified quiet background pixel and update the test fixture comment.

- [ ] **Step 8: Add Linux network collector**

In `host-usb/src/ip_detect.rs`, add:

```rust
#[cfg(target_os = "linux")]
pub fn collect_network_snapshot() -> std::io::Result<NetworkSnapshot> {
    let mut snapshot = NetworkSnapshot::default();
    collect_ipv4_addresses_getifaddrs(&mut snapshot)?;
    enrich_dynamic_flags_from_rtnetlink(&mut snapshot)?;
    collect_default_routes_rtnetlink(&mut snapshot)?;
    Ok(snapshot)
}
```

Implementation requirements:

- Use `libc::getifaddrs` for IPv4 addresses and interface flags.
- Set `is_up` from `IFF_UP`.
- Set `is_lower_up` from `IFF_RUNNING`; if unavailable on a platform, treat an `IFF_UP` non-loopback interface as lower-up.
- Use rtnetlink `RTM_GETADDR` to mark DHCP/dynamic candidates when the address is not permanent or has finite preferred/valid lifetimes.
- Use rtnetlink `RTM_GETROUTE` for IPv4 default routes where destination prefix length is `0`.
- Map interface indexes to names with `if_indextoname`.
- If rtnetlink dynamic detection fails, keep addresses with `is_dynamic = false` but still return addresses and routes; the pure selection rule will remain conservative for multiple isolated IPv4 addresses.

- [ ] **Step 9: Wire `run` in main**

Replace the `Command::Run` branch in `host-usb/src/main.rs`:

```rust
Ok(Command::Run(options)) => {
    miniboard_ipd::runtime::run_forever(options)?;
}
```

- [ ] **Step 10: Verify and hardware-test**

Run:

```powershell
cd host-usb
cargo test -- --nocapture
cargo build --release
```

On Linux target with the screen attached, run:

```bash
sudo ./target/release/miniboard-ipd run --interface eth0
```

Expected:

- No device: process sleeps and rescans every `500ms`.
- Device inserted: process opens the matching `1A86:FE0C` tty, handshakes, and shows pending page `3826`.
- Normal IPv4: process shows IP on page `3926` using two centered digit rows.
- Link-local only or ambiguous isolated IPv4 after `45s`: process shows DHCP failed page `3726`.
- USB removal: process logs disconnect, closes fd, returns to scanning, and reconnects cleanly after reinsertion.

- [ ] **Step 11: Commit**

Run:

```powershell
git add host-usb
git commit -m "feat: run host usb ip display daemon"
```

Expected: commit contains the Linux runtime loop, network collector, serial send path, keepalive, and hardware validation notes.

---

## Final Verification Checklist

After all tasks:

- [ ] Run `cd host-usb; cargo test -- --nocapture`.
- [ ] Run `cd host-usb; cargo build --release`.
- [ ] Run `cd flasher/src-tauri; cargo test`.
- [ ] Run `cd flasher; npm run build`.
- [ ] On a Linux target, run `miniboard-ipd run --interface <iface>` with the MSU2 MINI connected.
- [ ] Confirm pending page `3826` displays before IP selection.
- [ ] Confirm IP displays in two centered rows.
- [ ] Disconnect USB while running and confirm process returns to listening without crashing.
- [ ] Reconnect USB and confirm fresh handshake and redraw.
- [ ] Test generated service script for at least one systemd target and one small-device target if available.

## Execution Notes

- Do not claim the host daemon is complete until Task 10 is implemented and the Linux hardware checklist has been run.
- Real service file writes are exercised through `InstallOps` tests before `RealInstallOps` is wired into `main`.
- rtnetlink dynamic-address detection may need refinement after real Linux target testing; the pure selection rules are intentionally isolated so the collector can change without rewriting display/state logic.
