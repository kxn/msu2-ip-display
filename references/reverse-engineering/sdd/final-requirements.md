# Final Requirements Summary

Build a cross-platform-friendly Tauri/Rust desktop flasher for the MSU2/CH32x035 USB 0.96-inch screen stick.

Hardware/protocol constraints:
- Target USB VID/PID: `1A86:FE0C`.
- Serial settings: 19200 baud, 8N1, no flow control.
- Device handshake bytes: `00 4D 53 4E 43 4E`.
- ST7735S LCD is 160x80; flash image payload is 25600 bytes RGB565 big-endian.
- Flash plan must write fixed embedded assets only: offline animation frames at pages 0..3500 in 100-page steps, acquiring image at pages 3826..3925, IP background at pages 3926..4025, and must preserve page 4026+ font/resources.
- Writes must validate asset sizes before any flash erase/write and verify protocol replies during erase/write.
- After writing, preview should include and end on page 0 (`未连接`).

App requirements:
- Tauri app can detect the device, show concise device state, enable write only when a device is available, and run one-click write.
- UI must avoid exposing implementation details such as RGB565, page numbers, flash jargon, font page details, or design explanation text.
- User-facing copy should remain short and functional.
- Copyable log may contain technical details for debugging.
- No external asset pack feature is required.
- README should be concise user-facing usage notes, not the Tauri template.

Verification expected:
- Rust unit tests pass.
- Frontend production build passes.
- Hardware full write to COM4 reaches 3800/3800 pages and DONE.
- Page 0 preview command is confirmed after write.
- Tauri release build produces Windows installers.
