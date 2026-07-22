# Host Unflashed Runtime Rendering Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `--unflashed` host rendering support while removing the host text IP dependency on the flashed IP background page.

**Architecture:** Add a host `ResourceMode` that only affects full-screen status pages; text IP becomes a unified runtime renderer for both resource modes using official digit glyphs, `LCD_Load_RAM_Show`, direct-written dots, and a single runtime border. The flasher stops writing the host IP background page, while preserving any bytes still needed for the offline blank frame.

**Tech Stack:** Rust 2021, existing hand-written host CLI parser, existing MSU2 serial protocol packet builders, existing RGB565BE assets, shell installer tests.

## Global Constraints

- Default host behavior remains optimized for flashed boards.
- `--unflashed` is opt-in and accepted by `run` and `install`.
- The host daemon must not write project host resources into board flash.
- Text IP rendering must not depend on `IP_BACKGROUND_PAGE` or `LCD_Load_RAM_Mix_Show`.
- Official digit glyph pages at `4026 + digit` remain an expected board resource.
- QR rendering remains direct LCD writing and must behave the same in both resource modes.
- Pending/DHCP failed direct-write fallback is only for `ResourceMode::Unflashed`.
- Flasher no longer writes the host IP background page `500..599`.
- `ip_bg.rgb565be` must not be deleted until offline blank usage is handled.
- Tests must be written and observed failing before production implementation.

---

## File Structure

- `host-usb/src/cli.rs`: parse `--unflashed`, store `ResourceMode`, persist it in service args, update help.
- `host-usb/src/protocol.rs`: add `load_ram_show_packet()`.
- `host-usb/src/display.rs`: replace text IP rendering internals with runtime RAM-show, add border writes, add direct-write status renderers.
- `host-usb/src/runtime.rs`: route pending/DHCP by resource mode, keep text IP unified, update fake IO markers/tests.
- `host-usb/src/main.rs`: no behavior change expected beyond passing expanded `RunOptions`.
- `host-usb/README.md`, `README.md`, `docs/release-and-install.md`, `docs/msu2-protocol-and-flash-layout.md`: document `--unflashed`, runtime text rendering, and flash layout change.
- `scripts/test-install-miniboard-ipd.sh`: verify installer passes `--unflashed` through.
- `flasher/src-tauri/src/assets.rs`: remove the host IP background from the active flash plan while preserving offline blank bytes.
- `flasher/src-tauri/src/flasher.rs`: update tests/imports that assumed `HOST_IP_BG_PAGE` was in the plan or preview.
- `flasher/tools/generate_flash_assets.py`: rename or generate offline blank if needed so `ip_bg.rgb565be` is no longer conceptually host IP background.

---

### Task 1: CLI Resource Mode And Protocol Packet

**Files:**
- Modify: `host-usb/src/cli.rs`
- Modify: `host-usb/src/protocol.rs`

**Interfaces:**
- Produces: `pub enum ResourceMode { Flashed, Unflashed }`
- Produces: `RunOptions { resources: ResourceMode, ... }`
- Produces: `pub fn load_ram_show_packet() -> [u8; 6]`
- Consumed by later tasks: `Runtime::new(options, io)`, `DisplayRenderer`, service install argument embedding.

- [ ] **Step 1: Write failing CLI tests**

Add tests in `host-usb/src/cli.rs`:

```rust
#[test]
fn run_defaults_to_flashed_resources() {
    let command = parse_args(["run"]).unwrap();
    let Command::Run(options) = command else {
        panic!("expected run command");
    };

    assert_eq!(options.resources, ResourceMode::Flashed);
}

#[test]
fn unflashed_resource_mode_is_parsed_and_embedded_in_service_args() {
    let command = parse_args(["install", "--unflashed", "--interface", "eth0"]).unwrap();
    let Command::Install(options) = command else {
        panic!("expected install command");
    };

    assert_eq!(options.resources, ResourceMode::Unflashed);
    assert_eq!(
        options.service_args(),
        [
            "--interface",
            "eth0",
            "--dhcp-fail-delay-seconds",
            "45",
            "--unflashed",
        ]
    );
}

#[test]
fn unflashed_combines_with_qr_show_mode() {
    let command = parse_args(["run", "--unflashed", "--show", "qr"]).unwrap();
    let Command::Run(options) = command else {
        panic!("expected run command");
    };

    assert_eq!(options.resources, ResourceMode::Unflashed);
    assert_eq!(
        options.show,
        DisplayMode::Qr {
            template: DEFAULT_QR_TEMPLATE.to_string()
        }
    );
}
```

- [ ] **Step 2: Run CLI tests to verify red**

Run:

```powershell
cargo test --manifest-path host-usb/Cargo.toml cli::tests::unflashed -- --nocapture
```

Expected: compile failure or assertion failure because `ResourceMode` and `RunOptions::resources` do not exist.

- [ ] **Step 3: Implement minimal CLI support**

In `host-usb/src/cli.rs`, add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceMode {
    Flashed,
    Unflashed,
}
```

Add to `RunOptions`:

```rust
pub resources: ResourceMode,
```

Set the default in `parse_run_options`:

```rust
resources: ResourceMode::Flashed,
```

Handle the option:

```rust
"--unflashed" => {
    options.resources = ResourceMode::Unflashed;
}
```

Preserve it in `service_args()` after `--debug` and before `--show`:

```rust
if self.resources == ResourceMode::Unflashed {
    out.push("--unflashed".to_string());
}
```

Update `help_text()` options:

```text
  --unflashed                       Use runtime-rendered status screens for boards without project resources.
```

Update all existing test helper `RunOptions` literals to include `resources: ResourceMode::Flashed`.

- [ ] **Step 4: Verify CLI tests pass**

Run:

```powershell
cargo test --manifest-path host-usb/Cargo.toml cli::tests:: -- --nocapture
```

Expected: all CLI tests pass.

- [ ] **Step 5: Write failing protocol test**

Add in `host-usb/src/protocol.rs`:

```rust
#[test]
fn load_ram_show_packet_matches_official_demo() {
    assert_eq!(
        load_ram_show_packet(),
        [0x02, 0x03, 0x10, 0x00, 0x00, 0x00]
    );
}
```

- [ ] **Step 6: Run protocol test to verify red**

Run:

```powershell
cargo test --manifest-path host-usb/Cargo.toml protocol::tests::load_ram_show_packet_matches_official_demo -- --nocapture
```

Expected: compile failure because `load_ram_show_packet` does not exist.

- [ ] **Step 7: Implement protocol packet**

Add in `host-usb/src/protocol.rs`:

```rust
pub fn load_ram_show_packet() -> [u8; 6] {
    [0x02, 0x03, 0x10, 0x00, 0x00, 0x00]
}
```

- [ ] **Step 8: Verify protocol test passes**

Run:

```powershell
cargo test --manifest-path host-usb/Cargo.toml protocol::tests::load_ram_show_packet_matches_official_demo -- --nocapture
```

Expected: test passes.

- [ ] **Step 9: Commit task**

```powershell
git add host-usb/src/cli.rs host-usb/src/protocol.rs
git commit -m "feat(host): add unflashed resource mode"
```

---

### Task 2: Unified Runtime Text IP Renderer

**Files:**
- Modify: `host-usb/src/display.rs`
- Modify: `host-usb/src/runtime.rs` only where existing tests identify text IP by the old background page marker.

**Interfaces:**
- Consumes: `protocol::load_ram_show_packet()`
- Produces: `DisplayRenderer::ip(ip: Ipv4Addr) -> Vec<WireWrite>` using `LCD_Load_RAM_Show`
- Produces: direct border writes after the RAM-show packet

- [ ] **Step 1: Write failing display tests**

Add or replace tests in `host-usb/src/display.rs`:

```rust
#[test]
fn ip_render_uses_runtime_ram_show_instead_of_flash_background() {
    let writes = DisplayRenderer::ip(Ipv4Addr::new(192, 168, 1, 204));

    assert!(!writes
        .iter()
        .any(|write| write.bytes == show_photo_packet(IP_BACKGROUND_PAGE).to_vec()));
    assert!(!writes
        .iter()
        .any(|write| write.bytes == load_ram_mix_show_packet(IP_BACKGROUND_PAGE).to_vec()));
    assert!(writes
        .iter()
        .any(|write| write.bytes == ram_init_packet(0).to_vec()));
    assert!(writes
        .iter()
        .any(|write| write.bytes == load_ram_show_packet().to_vec()));
}

#[test]
fn ip_render_draws_dots_and_border_after_ram_show() {
    let writes = DisplayRenderer::ip(Ipv4Addr::new(10, 0, 1, 5));
    let ram_show_index = writes
        .iter()
        .position(|write| write.bytes == load_ram_show_packet().to_vec())
        .expect("expected LCD_Load_RAM_Show packet");
    let first_direct_lcd_after_ram_show = writes
        .iter()
        .enumerate()
        .skip(ram_show_index + 1)
        .find_map(|(index, write)| {
            (write.bytes == load_lcd_address_packet().to_vec()).then_some(index)
        })
        .expect("expected direct LCD writes after RAM show");

    assert!(first_direct_lcd_after_ram_show > ram_show_index);
    assert!(writes
        .iter()
        .skip(ram_show_index + 1)
        .any(|write| write.bytes == set_xy_packet(1, 1).to_vec()));
    assert!(writes
        .iter()
        .skip(ram_show_index + 1)
        .any(|write| write.bytes == set_xy_packet(158, 1).to_vec()));
}
```

Update imports in the test module to include `load_ram_show_packet`.

- [ ] **Step 2: Run display tests to verify red**

Run:

```powershell
cargo test --manifest-path host-usb/Cargo.toml display::tests::ip_render -- --nocapture
```

Expected: failure because current renderer still emits `show_photo_packet(IP_BACKGROUND_PAGE)` and `load_ram_mix_show_packet(IP_BACKGROUND_PAGE)`.

- [ ] **Step 3: Implement runtime text IP renderer**

In `host-usb/src/display.rs`:

Update imports:

```rust
use crate::protocol::{
    add_ram_masked_packet, load_lcd_address_packet, load_ram_show_packet, ram_init_packet,
    set_color_packet, set_size_packet, set_xy_packet, show_photo_packet, write_lcd_data_packet,
    DHCP_FAILED_PAGE, DIGIT_RESOURCE_PAGE, PENDING_PAGE,
};
```

Change `DisplayRenderer::ip` to:

```rust
pub fn ip(ip: Ipv4Addr) -> Vec<WireWrite> {
    let mut writes = vec![packet(ram_init_packet(0), false)];

    let layout = Self::layout_ip(ip);
    for glyph in layout.digits {
        let address = (DIGIT_RESOURCE_PAGE as u32 + glyph.digit as u32) * 256;
        writes.push(packet(set_xy_packet(glyph.x, glyph.y), false));
        writes.push(packet(set_size_packet(DIGIT_WIDTH, DIGIT_HEIGHT), false));
        writes.push(packet(add_ram_masked_packet(address), false));
    }

    writes.push(packet(set_color_packet(RGB565_TEXT, RGB565_BLACK), false));
    writes.push(packet(load_ram_show_packet(), false));

    for dot in layout.dots {
        writes.extend(dot_writes(dot));
    }
    writes.extend(border_writes());

    writes
}
```

Add helpers:

```rust
fn border_writes() -> Vec<WireWrite> {
    const SEGMENTS: &[(u16, u16, u16, u16)] = &[
        (1, 1, 158, 1),
        (1, 78, 158, 78),
        (1, 1, 1, 78),
        (158, 1, 158, 78),
        (8, 2, 28, 2),
        (3, 8, 3, 24),
        (131, 2, 151, 2),
        (156, 8, 156, 24),
        (8, 77, 28, 77),
        (3, 55, 3, 71),
        (131, 77, 151, 77),
        (156, 55, 156, 71),
    ];

    let mut writes = Vec::new();
    for &(x0, y0, x1, y1) in SEGMENTS {
        writes.extend(line_writes(x0, y0, x1, y1, RGB565_TEXT));
    }
    writes
}

fn line_writes(x0: u16, y0: u16, x1: u16, y1: u16, color: u16) -> Vec<WireWrite> {
    if y0 == y1 {
        let x = x0.min(x1);
        let width = x0.abs_diff(x1) + 1;
        filled_region_writes(x, y0, width, 1, color)
    } else if x0 == x1 {
        let y = y0.min(y1);
        let height = y0.abs_diff(y1) + 1;
        filled_region_writes(x0, y, 1, height, color)
    } else {
        panic!("runtime border only supports horizontal or vertical segments");
    }
}

fn filled_region_writes(x: u16, y: u16, width: u16, height: u16, color: u16) -> Vec<WireWrite> {
    let mut bytes = vec![0u8; width as usize * height as usize * 2];
    for pixel in bytes.chunks_exact_mut(2) {
        pixel.copy_from_slice(&color.to_be_bytes());
    }
    lcd_region_writes(x, y, width, height, &bytes)
}
```

- [ ] **Step 4: Update runtime fake IP marker if needed**

If `host-usb/src/runtime.rs` fake IO identifies text IP by `show_photo_packet(IP_BACKGROUND_PAGE)`, replace that marker with:

```rust
} else if writes
    .iter()
    .any(|write| write.bytes == crate::protocol::load_ram_show_packet().to_vec())
{
    "ip"
}
```

Remove any now-unused `IP_BACKGROUND_PAGE` import from runtime tests.

- [ ] **Step 5: Verify display and runtime tests pass**

Run:

```powershell
cargo test --manifest-path host-usb/Cargo.toml display::tests:: runtime::tests::tick_connects_device_and_renders_pending_then_ip -- --nocapture
```

Expected: display tests and selected runtime test pass.

- [ ] **Step 6: Commit task**

```powershell
git add host-usb/src/display.rs host-usb/src/runtime.rs
git commit -m "feat(host): render text ip without flashed background"
```

---

### Task 3: Unflashed Status Rendering In Runtime

**Files:**
- Modify: `host-usb/src/display.rs`
- Modify: `host-usb/src/runtime.rs`

**Interfaces:**
- Consumes: `ResourceMode`
- Produces: `DisplayRenderer::pending_runtime() -> Vec<WireWrite>`
- Produces: `DisplayRenderer::dhcp_failed_runtime() -> Vec<WireWrite>`
- Produces: runtime routing for `ShowPending` and `ShowDhcpFailed`

- [ ] **Step 1: Write failing renderer tests for runtime status screens**

Add in `host-usb/src/display.rs`:

```rust
#[test]
fn runtime_status_screens_direct_write_full_screen_images() {
    let pending = DisplayRenderer::pending_runtime();
    let failed = DisplayRenderer::dhcp_failed_runtime();

    for writes in [&pending, &failed] {
        assert_eq!(writes[0].bytes, set_xy_packet(0, 0).to_vec());
        assert_eq!(
            writes[1].bytes,
            set_size_packet(SCREEN_WIDTH, SCREEN_HEIGHT).to_vec()
        );
        assert_eq!(writes[2].bytes, load_lcd_address_packet().to_vec());
        assert_eq!(writes.len(), 103);
    }
}
```

- [ ] **Step 2: Run renderer test to verify red**

Run:

```powershell
cargo test --manifest-path host-usb/Cargo.toml display::tests::runtime_status_screens_direct_write_full_screen_images -- --nocapture
```

Expected: compile failure because runtime status renderer functions do not exist.

- [ ] **Step 3: Implement runtime status renderers**

In `host-usb/src/display.rs`, add:

```rust
const ACQUIRING_RGB565BE: &[u8] =
    include_bytes!("../../flasher/src-tauri/assets/acquiring.rgb565be");
const DHCP_FAILED_RGB565BE: &[u8] =
    include_bytes!("../../flasher/src-tauri/assets/dhcp_failed.rgb565be");
```

Add methods:

```rust
pub fn pending_runtime() -> Vec<WireWrite> {
    lcd_region_writes(0, 0, SCREEN_WIDTH, SCREEN_HEIGHT, ACQUIRING_RGB565BE)
}

pub fn dhcp_failed_runtime() -> Vec<WireWrite> {
    lcd_region_writes(0, 0, SCREEN_WIDTH, SCREEN_HEIGHT, DHCP_FAILED_RGB565BE)
}
```

- [ ] **Step 4: Verify renderer test passes**

Run:

```powershell
cargo test --manifest-path host-usb/Cargo.toml display::tests::runtime_status_screens_direct_write_full_screen_images -- --nocapture
```

Expected: test passes.

- [ ] **Step 5: Write failing runtime mode tests**

Add in `host-usb/src/runtime.rs` tests:

```rust
#[test]
fn unflashed_mode_direct_writes_pending_status_on_connect() {
    let start = Instant::now();
    let mut options = options();
    options.resources = ResourceMode::Unflashed;
    let io = FakeIo {
        now: Cell::new(Some(start)),
        devices: vec![target_device()],
        snapshot_results: VecDeque::from([Ok(Some(NetworkSnapshot::default()))]),
        ..FakeIo::default()
    };
    let mut runtime = Runtime::new(options, io);

    runtime.tick().unwrap();

    assert!(runtime
        .io
        .events
        .iter()
        .any(|event| event == "writes:pending_runtime"));
    assert!(!runtime.io.events.iter().any(|event| event == "writes:pending"));
}

#[test]
fn unflashed_mode_direct_writes_dhcp_failed_status() {
    let start = Instant::now();
    let mut options = options();
    options.resources = ResourceMode::Unflashed;
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
    let io = FakeIo {
        now: Cell::new(Some(start)),
        devices: vec![target_device()],
        snapshot_results: VecDeque::from([Ok(Some(link_local.clone())), Ok(Some(link_local))]),
        ..FakeIo::default()
    };
    let mut runtime = Runtime::new(options, io);

    runtime.tick().unwrap();
    runtime.io.set_now(start + Duration::from_secs(45));
    runtime.tick().unwrap();

    assert!(runtime
        .io
        .events
        .iter()
        .any(|event| event == "writes:dhcp_failed_runtime"));
}

#[test]
fn flashed_mode_keeps_page_based_pending_status() {
    let start = Instant::now();
    let io = FakeIo {
        now: Cell::new(Some(start)),
        devices: vec![target_device()],
        snapshot_results: VecDeque::from([Ok(Some(NetworkSnapshot::default()))]),
        ..FakeIo::default()
    };
    let mut runtime = Runtime::new(options(), io);

    runtime.tick().unwrap();

    assert!(runtime.io.events.iter().any(|event| event == "writes:pending"));
    assert!(!runtime
        .io
        .events
        .iter()
        .any(|event| event == "writes:pending_runtime"));
}
```

Update runtime test imports:

```rust
use crate::cli::{DisplayMode, ResourceMode};
```

Update `options()` helper with `resources: ResourceMode::Flashed`.

Update `FakeIo::send_writes()` markers:

```rust
let marker = if writes == DisplayRenderer::pending() {
    "pending"
} else if writes == DisplayRenderer::pending_runtime() {
    "pending_runtime"
} else if writes == DisplayRenderer::dhcp_failed() {
    "dhcp_failed"
} else if writes == DisplayRenderer::dhcp_failed_runtime() {
    "dhcp_failed_runtime"
} else if is_qr_writes(writes) {
    "qr"
} else if writes == DisplayRenderer::keepalive_white() {
    "keepalive_white"
} else if writes
    .iter()
    .any(|write| write.bytes == crate::protocol::load_ram_show_packet().to_vec())
{
    "ip"
} else {
    "other"
};
```

- [ ] **Step 6: Run runtime tests to verify red**

Run:

```powershell
cargo test --manifest-path host-usb/Cargo.toml runtime::tests::unflashed_mode -- --nocapture
```

Expected: failure because runtime always uses page-based pending/DHCP rendering.

- [ ] **Step 7: Implement runtime status routing**

In `host-usb/src/runtime.rs`, import `ResourceMode`:

```rust
use crate::cli::{DisplayMode, ResourceMode, RunOptions};
```

Add field to `Runtime<T>`:

```rust
resource_mode: ResourceMode,
```

Set it in `Runtime::new`:

```rust
let resource_mode = options.resources;
...
resource_mode,
```

Change `DaemonAction::ShowPending` branch:

```rust
let writes = match self.resource_mode {
    ResourceMode::Flashed => DisplayRenderer::pending(),
    ResourceMode::Unflashed => DisplayRenderer::pending_runtime(),
};
self.io.send_writes(&writes)?;
```

Change `DaemonAction::ShowDhcpFailed` similarly:

```rust
let writes = match self.resource_mode {
    ResourceMode::Flashed => DisplayRenderer::dhcp_failed(),
    ResourceMode::Unflashed => DisplayRenderer::dhcp_failed_runtime(),
};
self.io.send_writes(&writes)?;
```

Text IP branch stays:

```rust
self.io.send_writes(&DisplayRenderer::ip(*ip))?;
```

- [ ] **Step 8: Verify runtime tests pass**

Run:

```powershell
cargo test --manifest-path host-usb/Cargo.toml runtime::tests:: -- --nocapture
```

Expected: runtime tests pass.

- [ ] **Step 9: Commit task**

```powershell
git add host-usb/src/display.rs host-usb/src/runtime.rs
git commit -m "feat(host): direct write unflashed status screens"
```

---

### Task 4: Installer Pass-Through And User Docs

**Files:**
- Modify: `scripts/test-install-miniboard-ipd.sh`
- Modify: `scripts/install-miniboard-ipd.sh`
- Modify: `README.md`
- Modify: `host-usb/README.md`
- Modify: `docs/release-and-install.md`
- Modify: `docs/msu2-protocol-and-flash-layout.md`

**Interfaces:**
- Consumes: host binary accepts `--unflashed`
- Produces: installer tests proving pass-through
- Produces: user/developer docs for unflashed behavior

- [ ] **Step 1: Write failing installer pass-through test**

In `scripts/test-install-miniboard-ipd.sh`, add a test before the final test runner list:

```sh
test_unflashed_option_is_passed_to_service_install() {
  tmp=$(run_in_temp unflashed)
  fixture_dir=$tmp/fixtures
  fakebin=$tmp/fakebin
  install_root=$tmp/root
  log=$tmp/install.log
  curl_log=$tmp/curl.log
  mkdir -p "$fixture_dir"
  make_fixture linux-amd64 "$fixture_dir"
  make_fake_curl "$fakebin" "$fixture_dir"
  : > "$log"
  : > "$curl_log"

  PATH="$fakebin:$PATH" \
  MSU2_INSTALL_ROOT="$install_root" \
  MSU2_INSTALLER_ARCH=x86_64 \
  FAKE_LATEST_TAG=v0.0.0 \
  MSU2_RELEASE_BASE=https://example.invalid/releases/latest/download \
  INSTALL_LOG="$log" \
  CURL_LOG="$curl_log" \
    sh "$INSTALLER" --unflashed --interface eth0 > "$tmp/out"

  assert_contains "$install_root/usr/local/bin/miniboard-ipd install --unflashed --interface eth0" "$log"
}
```

Add to the runner list:

```sh
test_unflashed_option_is_passed_to_service_install
```

- [ ] **Step 2: Run installer test to verify red**

Run:

```powershell
sh scripts/test-install-miniboard-ipd.sh
```

Expected: failure until the fake binary or installer help paths are updated only if the installer rejects the option. If it passes immediately because the installer already forwards unknown args, keep the test as regression coverage and proceed.

- [ ] **Step 3: Update installer help**

In `scripts/install-miniboard-ipd.sh`, add an example:

```text
  curl -fsSL https://raw.githubusercontent.com/kxn/msu2-ip-display/master/scripts/install-miniboard-ipd.sh | sudo sh -s -- --unflashed
```

Add text:

```text
Host options such as --interface, --show, --debug, and --unflashed are passed to miniboard-ipd install.
```

- [ ] **Step 4: Update user docs**

In `README.md` and `host-usb/README.md`, add:

```markdown
如果没有先用 flasher 刷入项目资源，可以安装时加 `--unflashed`：

```sh
curl -fsSL https://raw.githubusercontent.com/kxn/msu2-ip-display/master/scripts/install-miniboard-ipd.sh | sudo sh -s -- --unflashed
```

这个模式不会改写小屏 flash。host 没连上时，小屏保持出厂/默认状态；host 连上后，状态页会直接写屏，切换比刷过资源的模式慢一点。
```

In `docs/release-and-install.md`, add:

```markdown
`--unflashed` is for boards that have not been flashed with project resources. The host embeds the acquiring-IP and DHCP-failed status screens and direct-writes them after connecting, while text IP rendering uses official digit glyphs and runtime border drawing.
```

In `docs/msu2-protocol-and-flash-layout.md`, add or update the protocol table row:

```markdown
| `02 03 10 00 00 00` | `LCD_Load_RAM_Show`; displays the monochrome RAM buffer using the currently configured foreground/background colors |
```

Update any active flash layout table so `500..599` is no longer described as required for host text IP after Task 5.

- [ ] **Step 5: Verify installer and docs checks**

Run:

```powershell
sh scripts/test-install-miniboard-ipd.sh
git diff --check
```

Expected: installer tests pass; diff check has no whitespace errors.

- [ ] **Step 6: Commit task**

```powershell
git add scripts/test-install-miniboard-ipd.sh scripts/install-miniboard-ipd.sh README.md host-usb/README.md docs/release-and-install.md docs/msu2-protocol-and-flash-layout.md
git commit -m "docs(host): document unflashed rendering mode"
```

---

### Task 5: Flasher Removes Host IP Background From Flash Plan

**Files:**
- Modify: `flasher/src-tauri/src/assets.rs`
- Modify: `flasher/src-tauri/src/flasher.rs`
- Modify: `flasher/tools/generate_flash_assets.py` if renaming/generating offline blank is needed
- Modify: `docs/msu2-protocol-and-flash-layout.md` if not completed in Task 4

**Interfaces:**
- Produces: `fixed_flash_plan()` without host IP background asset at `500..599`
- Preserves: offline blank frame bytes needed by resource directory/offline animation

- [ ] **Step 1: Write failing flasher asset tests**

Update tests in `flasher/src-tauri/src/assets.rs`:

```rust
#[test]
fn compact_plan_no_longer_writes_host_ip_background() {
    let assets = embedded_assets();
    let plan = fixed_flash_plan(&assets);

    assert!(!plan.iter().any(|asset| asset.label == "ip_bg"));
    assert!(!plan
        .iter()
        .any(|asset| asset.start_page == HOST_IP_BG_PAGE));
}
```

Update `compact_plan_writes_expected_assets_in_order` expected labels to remove `"ip_bg"` after the red run.

- [ ] **Step 2: Run flasher asset test to verify red**

Run:

```powershell
cargo test --manifest-path flasher/src-tauri/Cargo.toml assets::tests::compact_plan_no_longer_writes_host_ip_background -- --nocapture
```

Expected: failure because current plan includes `"ip_bg"` at `HOST_IP_BG_PAGE`.

- [ ] **Step 3: Remove host IP background from active plan**

In `flasher/src-tauri/src/assets.rs`, remove this `FlashAsset` from `fixed_flash_plan()`:

```rust
FlashAsset {
    label: "ip_bg",
    start_page: HOST_IP_BG_PAGE,
    page_count: RGB_IMAGE_PAGES,
    bytes: assets.ip_bg,
},
```

Keep `EmbeddedAssets::ip_bg` only if `offline_blank` still uses it:

```rust
FlashAsset {
    label: "offline_blank",
    start_page: OFFLINE_BLANK_PAGE,
    page_count: RGB_IMAGE_PAGES,
    bytes: assets.ip_bg,
},
```

Update label expectations:

```rust
vec![
    "offline_visible",
    "offline_blank",
    "offline_static",
    "pending",
    "dhcp_failed",
    "startup_logo",
    "resource_directory",
]
```

Remove assertions that expect `plan[5].start_page == HOST_IP_BG_PAGE`; shift startup logo/resource directory indexes accordingly.

- [ ] **Step 4: Update preview tests/imports**

In `flasher/src-tauri/src/flasher.rs`, `preview_pages()` currently previews `HOST_IP_BG_PAGE`. Change preview to avoid page `500`:

```rust
for page in [
    OFFLINE_VISIBLE_PAGE,
    HOST_PENDING_PAGE,
    OFFLINE_VISIBLE_PAGE,
] {
    ...
}
```

Update `preview_pages_sends_page_zero_last` expected photo packets from four packets to three packets.

Remove unused imports of `HOST_IP_BG_PAGE` where possible.

- [ ] **Step 5: Verify flasher tests pass**

Run:

```powershell
cargo test --manifest-path flasher/src-tauri/Cargo.toml assets::tests:: flasher::tests::preview_pages_sends_page_zero_last -- --nocapture
```

Expected: selected flasher tests pass.

- [ ] **Step 6: Update docs for active flash layout**

Update `docs/msu2-protocol-and-flash-layout.md` so `500..599` is not described as active host IP background. Use wording:

```markdown
| `500..599` | no longer written by the flasher for host text IP; text IP uses runtime RAM rendering |
```

If the doc has a flash plan list, remove the `ip_bg` item from the active plan.

- [ ] **Step 7: Commit task**

```powershell
git add flasher/src-tauri/src/assets.rs flasher/src-tauri/src/flasher.rs docs/msu2-protocol-and-flash-layout.md
git commit -m "feat(flasher): stop writing host ip background"
```

---

### Task 6: Full Verification And Hardware Smoke

**Files:**
- No production code expected.
- Test scripts or scratch files must not be committed unless they are reusable.

**Interfaces:**
- Consumes: all previous tasks.
- Produces: verified implementation ready for review.

- [ ] **Step 1: Run formatting**

Run:

```powershell
cargo fmt --manifest-path host-usb/Cargo.toml -- --check
cargo fmt --manifest-path flasher/src-tauri/Cargo.toml -- --check
```

Expected: both exit 0.

- [ ] **Step 2: Run host tests**

Run:

```powershell
cargo test --manifest-path host-usb/Cargo.toml -- --nocapture
```

Expected: all host tests pass.

- [ ] **Step 3: Run flasher tests**

Run:

```powershell
cargo test --manifest-path flasher/src-tauri/Cargo.toml -- --nocapture
```

Expected: all flasher tests pass.

- [ ] **Step 4: Run installer tests**

Run:

```powershell
sh scripts/test-install-miniboard-ipd.sh
```

Expected: installer tests pass.

- [ ] **Step 5: Run whitespace check**

Run:

```powershell
git diff --check
```

Expected: no whitespace errors.

- [ ] **Step 6: Optional hardware smoke**

If the board is still attached on `COM4`, run a temporary Python smoke script that:

- Opens `COM4` at `921600` with RTS/CTS.
- Handshakes with `\0MSNCN`.
- Sends the same sequence expected from the new text IP renderer for `10.0.1.5`.
- Leaves the screen showing the runtime-bordered IP.

Do not commit this scratch script unless it is converted into a documented reusable tool.

- [ ] **Step 7: Final commit if needed**

If Task 6 required small fixes:

```powershell
git add <changed-files>
git commit -m "test: verify unflashed host rendering"
```
