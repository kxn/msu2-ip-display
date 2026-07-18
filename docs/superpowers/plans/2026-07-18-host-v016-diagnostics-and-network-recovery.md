# Host v0.1.6 Diagnostics And Network Recovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship `v0.1.6` so field installs can prove the installed binary version and the host display daemon keeps updating correctly when Ethernet is unplugged or route netlink data is unavailable.

**Architecture:** Keep the existing single foreground daemon state machine, but make all Linux network enrichment best-effort. The installer must resolve a concrete release tag, download assets from that tag, and verify the installed binary reports the expected version.

**Tech Stack:** Rust 2021 for `host-usb`; POSIX `sh` installer; GitHub Releases assets.

## Global Constraints

- Host binary stays dependency-light and Linux-only.
- Installer remains one-line curl compatible and must support amd64, arm64, and arm32.
- Re-running the installer is an upgrade: stop old service first, replace binary atomically, then install/start the new service.
- No config files; service options are embedded from installer command arguments.
- Keep screen pages unchanged: pending page `300`, DHCP failed page `400`, IP background page `500`.

---

### Task 1: Version Reporting

**Files:**
- Modify: `host-usb/src/cli.rs`
- Modify: `host-usb/src/main.rs`

**Interfaces:**
- Produces: `Command::Version`
- Produces: `version_string() -> String`

- [ ] Write a failing CLI test for `--version` and `version`.
- [ ] Run the targeted CLI test and confirm it fails because `Command::Version` is missing.
- [ ] Add `Command::Version`, parser support, and `version_string()`.
- [ ] Print version from `main.rs` without touching service state.
- [ ] Run host tests.

### Task 2: Installer Release Resolution And Version Verification

**Files:**
- Modify: `scripts/install-miniboard-ipd.sh`
- Modify: `scripts/test-install-miniboard-ipd.sh`

**Interfaces:**
- Produces installer output `Resolved release vX.Y.Z`
- Produces installer output `Installed version: miniboard-ipd X.Y.Z`
- Uses `miniboard-ipd --version` for post-install verification.

- [ ] Write failing installer tests that require resolving `/releases/latest` to a concrete tag and downloading from `/releases/download/<tag>/...`.
- [ ] Write failing installer test that detects installed-version mismatch.
- [ ] Implement release tag resolution via `curl -w '%{url_effective}'`.
- [ ] Verify old and new binary versions are printed when possible.
- [ ] Run installer tests.

### Task 3: Network Snapshot Best-Effort Recovery

**Files:**
- Modify: `host-usb/src/ip_detect.rs`
- Modify: `host-usb/src/runtime.rs`

**Interfaces:**
- `collect_network_snapshot()` must return addresses even if dynamic or route rtnetlink enrichment fails.
- Runtime must feed an empty `NetworkSnapshot` after stale network worker errors so daemon can leave old IP/boot image states.

- [ ] Write failing tests for best-effort route failure and stale worker fallback.
- [ ] Make dynamic and route rtnetlink collection log-only on failure.
- [ ] Add a stale snapshot fallback timer in runtime.
- [ ] Reduce netlink receive timeout to `200ms`.
- [ ] Run host tests.

### Task 4: Release

**Files:**
- Modify: host/flasher version metadata to `0.1.6`.

- [ ] Run `cargo fmt --manifest-path host-usb/Cargo.toml -- --check`.
- [ ] Run `cargo fmt --manifest-path flasher/src-tauri/Cargo.toml -- --check`.
- [ ] Run `cargo test --manifest-path host-usb/Cargo.toml -- --nocapture`.
- [ ] Run `cargo test --manifest-path flasher/src-tauri/Cargo.toml -- --nocapture`.
- [ ] Run `npm run build` in `flasher`.
- [ ] Run `sh scripts/test-install-miniboard-ipd.sh`.
- [ ] Commit, push master, tag `v0.1.6`, push tag, wait for GitHub Release.
