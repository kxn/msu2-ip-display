# Flasher Screen Progress Design

## Goal

When the user starts flashing an MSU2 MINI, the device screen should visibly switch to a flashing status. If the firmware supports partial LCD writes on the current MINI path, the screen should also show coarse progress without repeatedly refreshing the whole 160x80 display.

## Sources

- Current flasher protocol: `flasher/src-tauri/src/protocol.rs`
- Current flashing loop: `flasher/src-tauri/src/flasher.rs`
- Existing screen assets: `flasher/src-tauri/assets/*.rgb565be`
- Official direct LCD API examples:
  - `references/vendor/source/msu2-lite-pro/MSU2_PRO_2.8_DemoV1.0.py`
  - `references/vendor/msu2-programmable-usb-screen/MSU2 演示Demo及固件V1.0/Python源码/MSU2_DemoV1.0.py`
- Mock images:
  - `docs/mockups/msu2-flash-status-contact-sheet.png`
  - `docs/mockups/msu2-flash-status-037.png`
  - `docs/mockups/msu2-flash-status-no-progress.png`
  - `docs/mockups/msu2-flash-status-done.png`

## Relevant Official Commands

The official Python demos expose direct LCD-area writes:

- `LCD_Set_XY(x, y)` sends `02 00 x_hi x_lo y_hi y_lo`
- `LCD_Set_Size(width, height)` sends `02 01 width_hi width_lo height_hi height_lo`
- `LCD_ADD(x, y, width, height)` sends `02 03 07 00 00 00` after setting XY and size
- `LCD_DATA(data, size)` sends 64 data groups of `04 index data0 data1 data2 data3`, then commits to LCD with `02 03 08 size_hi size_lo 00`

The current flasher already uses the same XY and size packet format for flash-page image display, so the only new protocol surface is `LCD_ADD` and `LCD_DATA`.

Hardware probe on `COM4` confirmed:

- `LCD_ADD` echoes `02 03 07 00 00 00`.
- `LCD_DATA` accepts data but does not emit a reply.
- The implementation must not wait for an `LCD_DATA` acknowledgement.

## Capability Rule

Direct LCD writes are treated as optional.

At the start of flashing, the app should attempt a small LCD capability probe. If the probe succeeds, screen status updates are enabled for that flash run. If it fails or times out, the app must continue flashing normally and only use the existing desktop UI progress.

Screen status failures must never turn a valid flash operation into a failed flash operation.

## Visual Design

The screen status keeps the existing MINI asset language:

- Black background
- Dark green subtle horizontal texture
- Neon green border
- Neon green Chinese state text
- Small bottom progress bar

The preferred progress-capable layout is:

- Title: `正在写入`
- Percentage: `0%` through `100%`
- Bottom progress bar at approximately `x=24, y=60, width=112, height=8`

The no-progress fallback layout is:

- Title: `正在写入`
- Subtitle: `请勿断开`

The done layout is:

- Title: `写入完成`
- Subtitle: `可拔下或重新连接`

The Rust backend should use pre-rendered RGB565 status fragments for Chinese text and percentage labels. Runtime font rendering is intentionally avoided.

## Partial Update Strategy

The implementation should avoid full-screen direct LCD refreshes during flashing.

Initial status may update only the regions needed for status:

- Title panel
- Percent panel
- Progress bar outline

Progress updates should be coarse and incremental:

- Do not update the screen for every flash page.
- Only update when the displayed bar advances by at least one pixel, and cap updates to percentage changes that are useful to a human.
- Prefer updating only the newly filled green strip inside the progress bar.
- If text percentage updates are too expensive or unstable on hardware, keep the bar-only progress and leave the desktop UI as the exact progress source.

## Flashing Flow

The desired flow is:

1. Open port and handshake, as today.
2. Run a small direct-LCD probe.
3. If the probe succeeds, draw the initial flashing status.
4. Flash existing fixed assets.
5. During flashing, update the device progress display with a throttled screen updater.
6. On success, optionally draw `写入完成`.
7. Run the existing `preview_pages` flow so the device returns to page `0`.

If any screen-status step fails, disable screen status updates for this run and continue at the next flash step.

## Testing Requirements

Automated tests must cover:

- New packet encodings for `LCD_ADD` and `LCD_DATA`.
- Direct LCD region writes chunk data into 256-byte LCD data packets.
- Screen status progress only writes incremental fill regions.
- Screen status disables itself after an LCD update failure.
- Existing flash write behavior and page preview behavior remain unchanged.

Manual hardware testing should cover:

- The device visibly shows `正在写入` when flashing starts.
- If direct LCD data works, the progress bar advances without full-screen flicker.
- Flashing still completes if the screen-status probe is disabled or fails.

## Out Of Scope

- Full custom font rendering in Rust.
- Full host-side graphics API implementation.
- Persisting the flashing status image into flash as a reusable resource.
- Cancel or pause support during flashing.
