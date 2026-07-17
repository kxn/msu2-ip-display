# Miniboard Reorganization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reorganize the MSU2 MINI flasher workspace into separate `flasher/`, `host-usb/`, `docs/`, and deduplicated tracked `references/` directories.

**Architecture:** Keep the existing Tauri/Vite/Rust flasher intact and move it as a complete app into `flasher/`. Add authored Markdown docs under `docs/`, reserve `host-usb/` for future communication code, and collect stable vendor/reverse-engineering inputs under `references/` with exact-file SHA-256 deduplication.

**Tech Stack:** PowerShell for filesystem moves and hash checks, Markdown documentation, existing Tauri 2/Vite/TypeScript/Rust flasher, npm build, Cargo tests.

## Global Constraints

- Do not change flasher protocol behavior during the directory reorganization.
- Track deduplicated reference binaries and documents in git.
- Keep generated dependency/build outputs untracked: `node_modules/`, `dist/`, `flasher/src-tauri/target/`, generated Tauri schemas, virtual environments, and temporary decompiler output.
- Preserve official reference files that differ by SHA-256, including both MINI flash firmware binaries.
- Verify with `npm run build` from `flasher/` and `cargo test` from `flasher/src-tauri/`.

---

### Task 1: Move The Flasher App

**Files:**
- Move: `index.html` -> `flasher/index.html`
- Move: `package.json` -> `flasher/package.json`
- Move: `package-lock.json` -> `flasher/package-lock.json`
- Move: `tsconfig.json` -> `flasher/tsconfig.json`
- Move: `vite.config.ts` -> `flasher/vite.config.ts`
- Move: `src/` -> `flasher/src/`
- Move: `src-tauri/` -> `flasher/src-tauri/`
- Move: `README.md` -> `flasher/README.md`
- Keep: `.gitignore` at repository root

**Interfaces:**
- Consumes: current flasher project at repository root.
- Produces: buildable flasher app rooted at `flasher/`.

- [ ] **Step 1: Create the destination directory**

Run: `New-Item -ItemType Directory -Force flasher`

Expected: `flasher/` exists.

- [ ] **Step 2: Move the tracked flasher files**

Run:

```powershell
Move-Item -LiteralPath index.html, package.json, package-lock.json, tsconfig.json, vite.config.ts, src, src-tauri, README.md -Destination flasher
```

Expected: the moved files are under `flasher/`; no tracked flasher app files remain at the root.

- [ ] **Step 3: Move local generated frontend state beside the app if present**

Run:

```powershell
if (Test-Path -LiteralPath node_modules) { Move-Item -LiteralPath node_modules -Destination flasher }
if (Test-Path -LiteralPath dist) { Move-Item -LiteralPath dist -Destination flasher }
```

Expected: any existing untracked `node_modules/` or `dist/` folders now sit under `flasher/`, where future npm commands expect them.

### Task 2: Add Repository And Future Host Documentation

**Files:**
- Create: `README.md`
- Create: `docs/README.md`
- Create: `docs/msu2-protocol-and-flash-layout.md`
- Create: `docs/flasher-notes.md`
- Create: `host-usb/README.md`

**Interfaces:**
- Consumes: official MSU2 guide, official Python reference code, previous final requirements, and hardware verification logs.
- Produces: human-readable docs that summarize useful facts without requiring readers to open binary references.

- [ ] **Step 1: Create documentation directories**

Run: `New-Item -ItemType Directory -Force docs, host-usb`

Expected: `docs/` and `host-usb/` exist.

- [ ] **Step 2: Write the new Markdown files**

Create the five files listed above with these responsibilities:

- `README.md`: repository map and quick commands.
- `docs/README.md`: docs index.
- `docs/msu2-protocol-and-flash-layout.md`: protocol packets, flash layout, official-source corrections, and uncertainty.
- `docs/flasher-notes.md`: current app behavior, verification evidence, and known limits.
- `host-usb/README.md`: scope marker for future host-side USB/serial code and protocol considerations.

Expected: docs are concise, Chinese-first where useful, and include exact source paths under `references/` where possible.

### Task 3: Deduplicate And Move Reference Material

**Files:**
- Create: `references/README.md`
- Create: `references/vendor/`
- Create: `references/reverse-engineering/`
- Create: `references/artifacts/`
- Move or copy selected files from: `data/`
- Move or copy selected files from: root `MSU2_MINI_DemoV1.6(1).exe`
- Move or copy selected files from: root `使用及开发指南-MSU2系列可编程USB屏幕_20241102224357.pdf`
- Copy selected useful files from: `.superpowers/sdd/`
- Copy selected useful files from: `codex-artifacts/outputs/`
- Copy selected useful files from: `codex-artifacts/work/`

**Interfaces:**
- Consumes: currently ignored local research/reference directories.
- Produces: deduplicated, git-trackable reference tree.

- [ ] **Step 1: Create reference directories**

Run:

```powershell
New-Item -ItemType Directory -Force references/vendor, references/vendor/source, references/vendor/firmware, references/vendor/tools, references/reverse-engineering, references/artifacts, references/artifacts/assets, references/artifacts/logs
```

Expected: reference directories exist.

- [ ] **Step 2: Move official vendor files**

Run native PowerShell `Move-Item`/`Copy-Item` commands to place:

- root official guide PDF under `references/vendor/docs/`
- root demo EXE under `references/vendor/tools/`
- `data/USB小屏幕固件更新流程.pdf` under `references/vendor/docs/`
- `data/Flash_MINI_V1.1(京东方屏幕).bin` under `references/vendor/firmware/`
- `data/Flash_MINI_V1.1(京东方屏幕)低速切换.bin` under `references/vendor/firmware/`
- extracted official `data/MSU2可编程USB副屏资料/...` tree under `references/vendor/msu2-programmable-usb-screen/`
- useful Python/C source files from the 1.47/2.8 source archive under `references/vendor/source/msu2-lite-pro/`

Expected: official reference material is no longer scattered at the repo root or under `data/`.

- [ ] **Step 3: Copy selected reverse-engineering and verification material**

Copy these selected files:

- `.superpowers/sdd/final-requirements.md`
- `.superpowers/sdd/progress.md`
- `.superpowers/sdd/task-8-report.md`
- `codex-artifacts/outputs/msu2_flash_tool_design.md`
- `codex-artifacts/work/msu2_guide_text.txt`
- `codex-artifacts/work/msu2_relevant_dis_clean.txt`
- `codex-artifacts/work/msu2_bytecode_summary.txt`
- `codex-artifacts/work/task8-hardware-verify.log`
- `codex-artifacts/work/task8-page0-preview-verify.log`
- `codex-artifacts/outputs/msu2_final_assets.zip`
- `codex-artifacts/outputs/msu2_final_*`
- `codex-artifacts/outputs/msu2_flasher_mock_v2_*`

Expected: selected materials are available under `references/reverse-engineering/` or `references/artifacts/`; dependency folders and temporary decompiler trees are not moved.

- [ ] **Step 4: Remove exact duplicate files in `references/` by SHA-256**

Run:

```powershell
$seen = @{}
Get-ChildItem -LiteralPath references -Recurse -File | ForEach-Object {
  $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $_.FullName).Hash
  if ($seen.ContainsKey($hash)) {
    Remove-Item -LiteralPath $_.FullName
  } else {
    $seen[$hash] = $_.FullName
  }
}
```

Expected: exact duplicate reference files are removed; distinct firmware binaries remain.

### Task 4: Update Ignore Rules And Git Visibility

**Files:**
- Modify: `.gitignore`
- Local check: `.git/info/exclude`

**Interfaces:**
- Consumes: new directory layout and existing local ignore rules.
- Produces: clean `git status` where intended references can be added.

- [ ] **Step 1: Update tracked ignore rules**

Ensure `.gitignore` ignores generated outputs in their new locations and does not ignore `references/`.

Expected reusable rules:

```gitignore
node_modules
dist
dist-ssr
*.local
```

plus existing editor/log ignores.

- [ ] **Step 2: Inspect local excludes**

Run: `git check-ignore -v references README.md docs host-usb flasher`

Expected: no output for paths that should be tracked.

- [ ] **Step 3: Add intended files**

Run: `git add -A`

Expected: moved tracked files, new docs, and deduplicated references are staged; generated ignored folders remain unstaged.

### Task 5: Verify The Reorganized Repository

**Files:**
- Read: `flasher/package.json`
- Read: `flasher/src-tauri/Cargo.toml`

**Interfaces:**
- Consumes: reorganized project.
- Produces: verification evidence for build and tests.

- [ ] **Step 1: Run frontend build**

Run: `npm run build` from `flasher/`.

Expected: command exits 0 and creates `flasher/dist/`.

- [ ] **Step 2: Run Rust tests**

Run: `cargo test` from `flasher/src-tauri/`.

Expected: command exits 0 with all tests passing.

- [ ] **Step 3: Inspect git status**

Run: `git status --short`

Expected: staged changes reflect the intended reorganization; generated folders remain ignored.

- [ ] **Step 4: Commit**

Run:

```powershell
git commit -m "chore: reorganize miniboard workspace"
```

Expected: commit succeeds with source, docs, and reference files included.
