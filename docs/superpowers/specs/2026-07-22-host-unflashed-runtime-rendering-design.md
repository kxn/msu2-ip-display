# Host Unflashed Runtime Rendering Design

## Context

The current host text IP display assumes the target MSU2 MINI has already been flashed by the flasher. In that mode, the host uses:

- `PENDING_PAGE = 300` for the acquiring-IP full-screen state.
- `DHCP_FAILED_PAGE = 400` for the DHCP-failed full-screen state.
- `IP_BACKGROUND_PAGE = 500` as the color background for text IP rendering.
- Official digit glyph pages at `4026 + digit`.

The new requirement is to support boards that have not been flashed with the project resources. This must be opt-in, because the default field flow still assumes flashed boards. When the host is not connected, it cannot control the display, so an unflashed board should remain in its default firmware state.

Hardware probing on 2026-07-22 confirmed that the host can use official digit glyph pages without writing project flash resources:

- The board enumerated as `VID:PID=1A86:FE0C` on `COM4`.
- `921600` baud with RTS/CTS and the `\0MSNCN` handshake worked.
- `LCD_RAM_Init` plus official digit glyph `4026 + digit` plus `LCD_Load_RAM_Show` displayed digits correctly.
- The probe did not issue flash erase or flash write packets.

## Goals

Add an opt-in runtime-resource mode for unflashed boards.

Keep the default mode optimized for already flashed boards.

Remove the host text IP dependency on the flashed IP background page.

Allow the flasher to stop writing the host IP background page.

Preserve the existing QR display behavior.

## Non-Goals

Do not write project host resources into board flash from the host daemon.

Do not change board behavior while the host is disconnected.

Do not remove the official digit glyph dependency; commodity boards are expected to contain those glyph pages.

Do not redesign the offline animation in this change, except where the flasher no longer writes the host IP background page.

Do not add a configuration file.

## Command Line

Add:

```text
miniboard-ipd run --unflashed
miniboard-ipd install --unflashed
```

Default behavior remains flashed mode.

`--unflashed` is accepted by both `run` and `install`. During `install`, it is preserved in `RunOptions::service_args()` so the generated service uses the same rendering mode after reboot.

`--unflashed` may be combined with:

- `--interface <name>`
- `--dhcp-fail-delay-seconds <n>`
- `--debug`
- `--show ip`
- `--show qr`
- `--show qr:<template>`

The help text must describe that `--unflashed` is for boards that have not been flashed with project resources and that display transitions are slower in this mode.

## Resource Mode Model

Add a resource mode to host run options:

```rust
pub enum ResourceMode {
    Flashed,
    Unflashed,
}
```

`RunOptions` gains:

```rust
pub resources: ResourceMode
```

The default is `ResourceMode::Flashed`.

## Text IP Rendering

Text IP rendering should no longer use `IP_BACKGROUND_PAGE` or `LCD_Load_RAM_Mix_Show`.

Both flashed and unflashed modes should use the same runtime text IP renderer:

1. Send `LCD_RAM_Init(0x00)` to clear the monochrome RAM buffer.
2. For each digit in the existing two-row IP layout:
   - Set `x/y`.
   - Set size to `24x33`.
   - Send `LCD_Add_RAM` using flash address `(4026 + digit) * 256`.
3. Send `LCD_Load_RAM_Show(text_color, background_color)`.
4. Draw dot glyphs with direct LCD writes.
5. Draw a runtime border with direct LCD writes.

This uses the official digit glyphs but not project-flashed background pages.

The first implementation should keep the current text color `0x5fd0` and background color `0x0000`.

### Protocol Additions

Add:

```rust
pub fn load_ram_show_packet() -> [u8; 6]
```

Expected bytes:

```text
02 03 10 00 00 00
```

`LCD_Load_RAM_Show` must be preceded by `set_color_packet(foreground, background)`.

Keep `load_ram_mix_show_packet(background_page)` for compatibility if other future paths need it, but the text IP renderer should stop using it.

## Runtime Border

The runtime border should approximate the existing `ip_bg.rgb565be` border, not the earlier double-border experiment.

Rules:

- One thin main frame.
- Four short corner accent lines.
- No second inner rectangle.
- Do not overlap the digit rows or the widest possible IP layout.
- Use the same RGB565 color as the text IP color.
- Draw after `LCD_Load_RAM_Show`, because `LCD_Load_RAM_Show` redraws the full display.

Recommended coordinates for the first implementation:

- Main frame:
  - Top: `(1, 1)` to `(158, 1)`
  - Bottom: `(1, 78)` to `(158, 78)`
  - Left: `(1, 1)` to `(1, 78)`
  - Right: `(158, 1)` to `(158, 78)`
- Corner accents:
  - Top-left horizontal: `(8, 2)` to `(28, 2)`
  - Top-left vertical: `(3, 8)` to `(3, 24)`
  - Top-right horizontal: `(131, 2)` to `(151, 2)`
  - Top-right vertical: `(156, 8)` to `(156, 24)`
  - Bottom-left horizontal: `(8, 77)` to `(28, 77)`
  - Bottom-left vertical: `(3, 55)` to `(3, 71)`
  - Bottom-right horizontal: `(131, 77)` to `(151, 77)`
  - Bottom-right vertical: `(156, 55)` to `(156, 71)`

The exact coordinates can be tuned during implementation if tests or hardware verification show overlap. The important design constraint is a single-frame look that keeps all direct-written border pixels outside the widest digit layout. This differs from the old flashed background, where the border was underneath the digits and could safely occupy more central pixels.

## Pending And DHCP-Failed Rendering

Pending and DHCP-failed pages are still full-screen status images.

In `ResourceMode::Flashed`:

- `ShowPending` uses `show_photo_packet(PENDING_PAGE)`.
- `ShowDhcpFailed` uses `show_photo_packet(DHCP_FAILED_PAGE)`.

In `ResourceMode::Unflashed`:

- `ShowPending` direct-writes a host-embedded 160x80 RGB565BE acquiring-IP image.
- `ShowDhcpFailed` direct-writes a host-embedded 160x80 RGB565BE DHCP-failed image.

The host binary can embed these two existing assets:

- `flasher/src-tauri/assets/acquiring.rgb565be`
- `flasher/src-tauri/assets/dhcp_failed.rgb565be`

This increases the host binary by about 50 KB before compression, which is acceptable for the target use case. It keeps status screen visuals consistent while avoiding flash writes.

## QR Rendering

QR rendering is already runtime direct LCD writing.

`--show qr` and `--show qr:<template>` behave the same in both resource modes. They do not need flashed project resources.

The QR keepalive remains white.

## Runtime Behavior

Disconnected state:

- The host does not send display writes.
- An unflashed board remains in its default firmware state.
- A flashed board remains controlled only after the host reconnects and handshakes.

On successful handshake:

- `ResourceMode::Flashed` displays pending through the flashed page.
- `ResourceMode::Unflashed` displays pending through direct LCD write.

When IPv4 becomes displayable:

- `DisplayMode::Text` uses the unified runtime text IP renderer in both resource modes.
- `DisplayMode::Qr` uses the existing QR renderer.

On DHCP failure:

- Rendering follows the resource mode split described above.

On disconnect:

- The host closes the serial session and returns to scanning.
- It does not attempt to draw an offline page.

## Flasher Changes

The flasher no longer needs to write the host IP background page `500..599`.

Implementation should remove the `ip_bg` item from `fixed_flash_plan()`.

The `HOST_IP_BG_PAGE` constant can be removed if no tests or preview code need it after the plan change. If removal creates unnecessary churn, it can remain briefly as a deprecated constant, but no active flash plan or host text IP path should depend on it.

The `ip_bg.rgb565be` file needs separate handling because it is currently also used as the `offline_blank` frame in the compact flash plan. The implementation should not silently delete the file if the offline animation still depends on it.

Preferred cleanup:

- Stop treating `ip_bg.rgb565be` as a host IP background.
- If the blank offline frame still needs the same bytes, either rename it to `offline_blank.rgb565be` or generate the blank frame in code.
- Update tests and docs so page `500..599` is no longer listed as an active host IP background resource.

The release artifact still includes a flasher that writes offline, pending, DHCP-failed, logo, and resource-directory assets. It just no longer writes a host IP background.

## Documentation Changes

Update user-facing docs:

- Default install assumes a flashed board.
- Use `--unflashed` for boards that have not been flashed with project resources.
- `--unflashed` keeps disconnected board behavior unchanged.
- `--unflashed` may have slower pending/DHCP transitions because those screens are direct-written.
- Text IP rendering no longer requires the flashed host IP background page.

Update developer docs:

- Protocol docs include `LCD_Load_RAM_Show`.
- Flash layout docs remove active use of page `500..599`.
- Flasher docs explain that the host IP background page is no longer written.

## Testing Plan

CLI tests:

- `run` defaults to `ResourceMode::Flashed`.
- `run --unflashed` sets `ResourceMode::Unflashed`.
- `install --unflashed` preserves `--unflashed` in `service_args()`.
- `--unflashed` combines with `--show qr`, `--interface`, `--debug`, and DHCP delay.
- Help text mentions `--unflashed`.

Protocol tests:

- `load_ram_show_packet()` returns `02 03 10 00 00 00`.

Display renderer tests:

- Text IP renderer does not emit `show_photo_packet(IP_BACKGROUND_PAGE)`.
- Text IP renderer does not emit `load_ram_mix_show_packet(IP_BACKGROUND_PAGE)`.
- Text IP renderer emits `ram_init_packet(0)`.
- Text IP renderer emits official digit glyph `add_ram_masked_packet((4026 + digit) * 256)`.
- Text IP renderer emits `load_ram_show_packet()`.
- Dot writes and border writes occur after `load_ram_show_packet()`.
- Border writes use single-frame coordinates, not the double-border experiment.

Runtime tests:

- Flashed pending uses page display.
- Unflashed pending uses full-screen direct LCD writes.
- Flashed DHCP failed uses page display.
- Unflashed DHCP failed uses full-screen direct LCD writes.
- Text IP uses the same renderer in both resource modes.
- QR mode is unchanged by `--unflashed`.

Flasher tests:

- `fixed_flash_plan()` no longer includes `HOST_IP_BG_PAGE`.
- Plan labels no longer include `"ip_bg"` as a host IP background.
- Protected page validation still passes.
- The offline blank frame remains valid after any asset rename or generation.

Installer script tests:

- Passing `--unflashed` through the installer results in `miniboard-ipd install --unflashed ...`.

Hardware verification:

- On a board without project resources, run `miniboard-ipd run --unflashed`.
- Confirm pending direct-write appears after connect.
- Confirm text IP appears with official digit glyphs and a single runtime border.
- Confirm DHCP failed direct-write appears after the configured delay.
- Confirm disconnect returns control to the board default state.
- Confirm QR mode still scans.

## Risks And Mitigations

Risk: Official digit glyph pages differ on some boards.

Mitigation: The mode still targets commodity boards that ship with the official glyph pages. Document this assumption.

Risk: Runtime text IP visual differs from the old `ip_bg` asset.

Mitigation: Use a single-frame border approximation and verify on hardware. The old fully flashed status pages remain for pending and DHCP failed.

Risk: Direct-writing status images can be slower than `show_photo`.

Mitigation: Keep direct-write status images only for `--unflashed`. Default flashed mode keeps fast status page display.

Risk: Removing `ip_bg` from the flasher plan affects offline blank animation if the file is deleted too aggressively.

Mitigation: Treat host IP background removal and offline blank asset handling as separate steps. Keep or rename the bytes needed by offline blank.

## Implementation Sequence

1. Add `ResourceMode` and `--unflashed` CLI parsing/service persistence.
2. Add `load_ram_show_packet()`.
3. Replace text IP rendering with runtime RAM-show rendering and border drawing.
4. Split pending/DHCP renderers by resource mode.
5. Embed or expose acquiring and DHCP-failed RGB565 assets to host runtime.
6. Update runtime action selection and keepalive behavior.
7. Update installer tests for `--unflashed` pass-through.
8. Remove host IP background from the flasher flash plan and adjust asset naming/reuse.
9. Update README, host README, release/install docs, and protocol/flash-layout docs.
10. Run unit tests, installer tests, and hardware smoke tests.
