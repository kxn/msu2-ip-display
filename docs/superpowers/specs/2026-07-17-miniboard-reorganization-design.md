# Miniboard Repository Reorganization Design

Date: 2026-07-17

Status: Approved for implementation

## Goal

Organize the current MSU2 MINI flasher research workspace into a maintainable repository layout that separates product code, future host-side communication code, useful project documentation, and reference material.

## Current State

The repository root currently contains the Tauri/Vite/Rust flasher project, official vendor资料 under `data/`, earlier reverse-engineering and build artifacts under `codex-artifacts/`, and root-level vendor binaries/PDFs. The actual flasher code is the tracked Vite/Tauri project made up of `src/`, `src-tauri/`, `index.html`, `package.json`, `package-lock.json`, `tsconfig.json`, and `vite.config.ts`.

The current flasher writes fixed embedded assets to a connected MSU2 MINI device. It has been hardware-verified on `COM4` with VID/PID `1A86:FE0C`, handshake bytes `00 4D 53 4E 43 4E`, and a full 3800-page flash write followed by page 0 preview. Later hardware probing also confirmed the official high-speed serial path, `921600 8N1` with RTS/CTS enabled.

## Target Layout

```text
flasher/
  src/
  src-tauri/
  index.html
  package.json
  package-lock.json
  tsconfig.json
  vite.config.ts

host-usb/
  README.md

docs/
  README.md
  msu2-protocol-and-flash-layout.md
  flasher-notes.md
  superpowers/
    specs/
      2026-07-17-miniboard-reorganization-design.md

references/
  README.md
  vendor/
  reverse-engineering/
  artifacts/
```

## Directory Responsibilities

`flasher/` contains the existing Tauri app. The move must preserve build behavior by updating relative paths only where required by the new working directory. The app should continue to build from `flasher/` with `npm run build` and Rust tests should continue to run from `flasher/src-tauri/`.

`host-usb/` is an intentionally small reserved stub for future host-side USB/serial communication work. It should contain a README explaining that no implementation exists yet and summarizing the key protocol facts that future code should consider.

`docs/` contains authored, useful documents. These files should be concise Markdown summaries that can be read without opening binary references. The first useful document must cover protocol packets, flash layout, confirmed facts, and uncertainty introduced by the newer official reference code.

`references/` contains source/reference material, including official PDFs, firmware binaries, vendor Python/C source, selected reverse-engineering notes, hardware verification logs, and previously generated useful assets. These files are expected to be tracked in git after deduplication.

## Reference Deduplication

Reference material should be deduplicated before it is moved into `references/`.

Rules:

- Keep one copy of each exact duplicate by SHA-256.
- Prefer the more official or more descriptive filename when duplicate content exists.
- Keep both `Flash_MINI_V1.1(京东方屏幕).bin` and `Flash_MINI_V1.1(京东方屏幕)低速切换.bin` because their SHA-256 hashes differ, even though they only differ near the end of the file.
- Do not copy dependency folders, build folders, virtual environments, or decompiled temporary trees into `references/`.
- Keep selected logs and summaries that explain verified behavior, especially full hardware write evidence and page 0 preview evidence.

## Documentation Corrections From Official Material

The new docs should correct or qualify earlier reverse-engineered assumptions:

- Earlier reverse-engineering used a verified `19200 8N1` path, but the official 1.47-inch and 2.8-inch reference Python code opens serial at `921600` with RTS/CTS enabled, and that high-speed mode has now been confirmed on the current MINI. Future host-side code should still support per-device or negotiated serial settings instead of assuming one value for every model or firmware.
- Flash storage is 1024KB with 4096 pages of 256 bytes each.
- The MINI/160x80 color image size is `160 * 80 * 2 = 25600` bytes, or 100 flash pages.
- The flasher performs a partial fixed asset write, not a full 1MB factory flash restore.
- Current fixed write layout is page `0..3599` for 36 offline animation frames, page `3826..3925` for acquiring background, page `3926..4025` for IP background, and page `4026+` preserved.
- Official docs describe 36 offline animation frames at 100ms intervals and distinguish full flash firmware restoration from custom image/background writes.

## Git Tracking Strategy

The reorganized source, docs, and deduplicated references should be committed. Large reference binaries are acceptable because they are stable research inputs and not expected to churn.

Generated dependency/build outputs must stay untracked:

- `node_modules/`
- `dist/`
- `flasher/src-tauri/target/`
- `flasher/src-tauri/gen/schemas/`
- virtual environments and temporary decompiler output

The local `.git/info/exclude` currently ignores `data/`, `codex-artifacts/`, the root EXE, the root PDF, and `.superpowers/`. Since `.git/info/exclude` is local-only, implementation should either use `git add -f` for the selected reference files or remove stale local exclude entries after the move. The tracked `.gitignore` should be updated only for reusable project ignore rules.

## Verification

After reorganizing:

- Run `npm run build` from `flasher/`.
- Run `cargo test` from `flasher/src-tauri/`.
- Verify `git status --short` shows the intended moved files, new docs, and deduplicated references.
- Do not require hardware reflashing for this directory-only change.
