# Flasher Screen Progress Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add optional MSU2 MINI on-device flashing status and throttled progress display during fixed-asset flashing.

**Architecture:** Add protocol primitives for direct LCD region writes, then add a small screen-status module that renders only status rectangles and progress-bar deltas. Wire it into `start_flash` as a best-effort optional feature so failures disable screen updates without failing flash writes.

**Tech Stack:** Rust/Tauri backend, existing `serialport`-based `PortIo`, existing Vite frontend progress events, pre-generated RGB565 mock/design assets.

## Global Constraints

- Keep the existing flash asset layout unchanged.
- Do not write to preserved font/resource pages.
- Do not repeatedly direct-write the full 160x80 screen during flashing.
- Continue flashing if screen-status probe or updates fail.
- Test production Rust behavior before implementation changes are accepted.

---

### Task 1: Direct LCD Protocol Packets

**Files:**
- Modify: `flasher/src-tauri/src/protocol.rs`

**Interfaces:**
- Produces: `load_lcd_address_packet() -> [u8; 6]`
- Produces: `write_lcd_data_packet(size: u16, data: &[u8; 256]) -> [u8; 390]`
- Produces: `expected_lcd_data_reply(size: u16) -> [u8; 6]`

- [ ] **Step 1: Write failing protocol tests**

Add tests showing:

```rust
assert_eq!(load_lcd_address_packet(), [0x02, 0x03, 0x07, 0x00, 0x00, 0x00]);
assert_eq!(
    expected_lcd_data_reply(16),
    [0x02, 0x03, 0x08, 0x00, 0x10, 0x00]
);
let data = [0x5a; 256];
let packet = write_lcd_data_packet(16, &data);
assert_eq!(packet.len(), 390);
assert_eq!(&packet[0..6], &[0x04, 0x00, 0x5a, 0x5a, 0x5a, 0x5a]);
assert_eq!(&packet[384..390], &[0x02, 0x03, 0x08, 0x00, 0x10, 0x00]);
```

- [ ] **Step 2: Run test and confirm RED**

Run:

```powershell
cd flasher/src-tauri
cargo test protocol::tests::lcd_direct_write_packets_match_official_demo
```

Expected: compile failure because the new functions do not exist.

- [ ] **Step 3: Implement packet helpers**

Add helpers that mirror the existing flash page packet structure, except the commit footer is `02 03 08 size_hi size_lo 00`.

- [ ] **Step 4: Run test and confirm GREEN**

Run the same cargo test. Expected: the new protocol test passes.

### Task 2: Direct LCD Region Writer

**Files:**
- Modify: `flasher/src-tauri/src/flasher.rs`

**Interfaces:**
- Consumes: protocol helpers from Task 1
- Produces: `write_lcd_region(port, x, y, width, height, bytes, retry) -> AppResult<()>`

- [ ] **Step 1: Write failing tests**

Add tests that call `write_lcd_region` on `MockPort` and assert:

- It sends `set_xy_packet(x, y)`.
- It sends `set_size_packet(width, height)`.
- It sends `load_lcd_address_packet()`.
- It sends one or more LCD data packets.
- It rejects `bytes.len() != width * height * 2`.

- [ ] **Step 2: Run tests and confirm RED**

Run:

```powershell
cd flasher/src-tauri
cargo test flasher::tests::lcd_region_writer
```

Expected: compile failure because `write_lcd_region` does not exist.

- [ ] **Step 3: Implement minimal region writer**

Implement direct LCD write with 256-byte chunks padded with `0xFF` for the final partial chunk. Use `expected_lcd_data_reply(size)` for reply matching.

- [ ] **Step 4: Run tests and confirm GREEN**

Run the same cargo test. Expected: region writer tests pass.

### Task 3: Best-Effort Screen Status State Machine

**Files:**
- Create: `flasher/src-tauri/src/screen_status.rs`
- Modify: `flasher/src-tauri/src/lib.rs`
- Modify: `flasher/src-tauri/src/flasher.rs`

**Interfaces:**
- Produces: `ScreenStatus<P>`
- Produces: `ScreenStatus::probe(port: &mut P) -> Self`
- Produces: `ScreenStatus::start(&mut self, port: &mut P)`
- Produces: `ScreenStatus::update(&mut self, port: &mut P, percent: u8)`
- Produces: `ScreenStatus::finish(&mut self, port: &mut P)`

- [ ] **Step 1: Write failing tests**

Add tests that assert:

- Probe failure returns a disabled status object.
- Update after disabled probe sends no writes.
- Progress updates send only when percent maps to a new bar pixel.
- A write failure disables later screen updates.

- [ ] **Step 2: Run tests and confirm RED**

Run:

```powershell
cd flasher/src-tauri
cargo test screen_status
```

Expected: compile failure because the module does not exist.

- [ ] **Step 3: Implement minimal screen status**

Use fixed rectangles:

- Title panel around the center of the screen.
- Percent panel under the title.
- Progress bar `x=24, y=60, width=112, height=8`.

Render RGB565 big-endian rectangles in Rust with the same green/black palette used in the mock. Use partial LCD writes only.

- [ ] **Step 4: Run tests and confirm GREEN**

Run the same cargo test. Expected: screen status tests pass.

### Task 4: Integrate With Flash Flow

**Files:**
- Modify: `flasher/src-tauri/src/commands.rs`
- Modify: `flasher/src-tauri/src/flasher.rs`

**Interfaces:**
- Consumes: `ScreenStatus`
- Preserves: existing `flash-progress` events
- Preserves: existing `preview_pages` on success

- [ ] **Step 1: Write failing integration test**

Add a unit-level test around the flash helper path proving that screen status failures do not fail the flash run.

- [ ] **Step 2: Run tests and confirm RED**

Run:

```powershell
cd flasher/src-tauri
cargo test flasher::tests::screen_status_failure_does_not_fail_flash
```

Expected: failure before integration exists.

- [ ] **Step 3: Wire screen status into flashing**

After handshake and before `flash_images`, create/probe screen status, draw initial status if enabled, pass updates from existing `FlashProgress`, then draw finish state before preview.

- [ ] **Step 4: Run tests and confirm GREEN**

Run targeted tests, then full Rust tests.

### Task 5: Hardware Probe And Final Verification

**Files:**
- Modify docs if hardware behavior differs from the design.

**Interfaces:**
- Produces: hardware notes in `docs/flasher-notes.md` or the design spec if direct LCD writes fail.

- [ ] **Step 1: Build and run automated verification**

Run:

```powershell
cd flasher
npm run build
cd src-tauri
cargo test
```

- [ ] **Step 2: Run hardware flash test**

With the MINI connected, start the flasher and verify:

- The device shows a flashing status near the start.
- Progress advances if direct LCD writes are supported.
- If screen status fails, flashing still completes and the desktop UI remains correct.

- [ ] **Step 3: Commit**

Commit implementation, docs, and mock assets with:

```powershell
git add -A
git commit -m "feat: show on-device flash progress"
```
