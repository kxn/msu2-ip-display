# Task 9 Report

Date: 2026-07-17

## What I implemented

- Added service installation application primitives in `host-usb/src/service_install.rs`:
  - `InstallRequest`
  - `InstallOps`
  - `RealInstallOps`
  - `install_commands`
  - `uninstall_commands`
  - `status_command`
  - `apply_install`
  - `apply_uninstall`
  - `run_status`
- Added command-level tests for install, uninstall, and status behavior with recording ops.
- Wired the Linux `main.rs` path so:
  - `install` copies the current executable to `/usr/local/bin/miniboard-ipd`, writes the detected init service file, and runs the init-specific enable/start commands.
  - `uninstall` runs the init-specific stop/disable commands and removes the generated service file plus installed binary.
  - `status` runs the init-specific status command.
  - `run` remains the Task 10 placeholder.
- Preserved the non-Linux unsupported-platform behavior.
- Kept CLI parse errors on exit code 2 by mapping them to `io::ErrorKind::InvalidInput` in Linux `main.rs`, avoiding any need to change `cli.rs`.
- Updated:
  - `host-usb/README.md`
  - `README.md`
  - `docs/README.md`

## TDD RED command and failing output summary

Command:

```powershell
cd host-usb
cargo test service_install::command_tests -- --nocapture
```

Observed failing summary before implementation:

- 5 command tests ran.
- 1 passed: `systemd_install_commands_reload_enable_and_start`
- 4 failed:
  - `apply_openwrt_install_writes_executable_init_script`
  - `apply_systemd_install_copies_binary_writes_unit_and_starts_service`
  - `status_runs_init_specific_command`
  - `uninstall_stops_service_removes_script_and_binary`
- Failure shape matched the stubs:
  - `apply_install` produced no recorded copy/write/run events
  - `apply_uninstall` produced no recorded run/remove events
  - `run_status` produced no recorded status command

## GREEN/final verification commands and results

Targeted GREEN verification:

```powershell
cd host-usb
cargo test service_install::command_tests -- --nocapture
```

Result:

- 5 passed
- 0 failed

Final required verification:

```powershell
cd host-usb
cargo test -- --nocapture
cargo build --release
```

Results:

- `cargo test -- --nocapture`: 42 passed, 0 failed
- `cargo build --release`: succeeded

## Commit hash

- `496b56b` - `feat: install host usb daemon service`

## Self-review notes or concerns

- `docs/README.md` briefly picked up an encoding regression during editing; I restored the original UTF-8 text and re-applied only the requested new link before commit.
- `RealInstallOps` executes real service-management commands and file writes, but the tests intentionally exercise behavior through `RecordingOps` rather than touching the host system.
- `command_exists` currently checks `PATH` entries by simple existence, which matches the task scope and current main-path needs.

## Fix after review

### RED

Focused commands:

```powershell
cd host-usb
cargo test cli::tests -- --nocapture
cargo test service_install::command_tests -- --nocapture
```

Observed failures before the fix:

- `cli::tests::install_rejects_invalid_interface_names_for_service_embedding`
  - `parse_args(["install", "--interface", ""])` still returned the old non-empty error and did not enforce the Linux-safe service embedding restrictions.
- `service_install::command_tests::uninstall_stops_service_removes_script_and_binary`
  - Recorded event order was:
    - `run:systemctl disable --now miniboard-ipd.service`
    - `run:systemctl daemon-reload`
    - `remove:/etc/systemd/system/miniboard-ipd.service`
    - `remove:/usr/local/bin/miniboard-ipd`
  - That proved `daemon-reload` ran before unit removal.

### GREEN

Focused verification after the fix:

```powershell
cd host-usb
cargo test cli::tests -- --nocapture
cargo test service_install::command_tests -- --nocapture
```

Results:

- `cli::tests`: 6 passed, 0 failed
- `service_install::command_tests`: 6 passed, 0 failed

Final required verification:

```powershell
cd host-usb
cargo test -- --nocapture
cargo build --release
```

Results:

- `cargo test -- --nocapture`: 45 passed, 0 failed
- `cargo build --release`: succeeded
