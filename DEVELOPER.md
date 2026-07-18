# Developer Guide

这里整理项目开发、验证和资料入口。面向想改代码、查协议、重新打包或继续逆向/整理资源的人。

## 仓库目录

| 目录 | 内容 |
| --- | --- |
| `flasher/` | 桌面刷写器，Tauri + Rust |
| `host-usb/` | Linux host 端 IP 显示服务 |
| `docs/` | 协议、资源布局、需求和设计文档 |
| `references/` | 官方资料、调研资料和验证材料 |
| `scripts/` | 安装脚本和脚本测试 |

## 常用验证

host 端：

```sh
cargo fmt --manifest-path host-usb/Cargo.toml -- --check
cargo test --manifest-path host-usb/Cargo.toml -- --nocapture
sh scripts/test-install-miniboard-ipd.sh
```

flasher：

```sh
cargo fmt --manifest-path flasher/src-tauri/Cargo.toml -- --check
cargo test --manifest-path flasher/src-tauri/Cargo.toml -- --nocapture
cd flasher && npm run build
```

提交前再跑一次空白检查：

```sh
git diff --check
```

## 主要文档

| 文档 | 内容 |
| --- | --- |
| [docs/msu2-protocol-and-flash-layout.md](docs/msu2-protocol-and-flash-layout.md) | MSU2 协议和 Flash 资源布局 |
| [docs/release-and-install.md](docs/release-and-install.md) | GitHub Actions、Release 和安装脚本 |
| [docs/superpowers/specs/2026-07-17-host-usb-ip-display-design.md](docs/superpowers/specs/2026-07-17-host-usb-ip-display-design.md) | host 端 IP 显示程序设计 |
| [docs/superpowers/specs/2026-07-18-host-usb-qr-display-design.md](docs/superpowers/specs/2026-07-18-host-usb-qr-display-design.md) | host 端二维码显示模式设计 |

## 发布

推送到 `master` 会触发 CI，生成 host 和 flasher 的 artifacts。打 `v*` tag 会触发 Release workflow，并把产物上传到对应 GitHub Release。

当前发布产物包括：

| 用途 | 文件 |
| --- | --- |
| Linux host 程序 x86_64 | `miniboard-ipd-linux-amd64.tar.gz` |
| Linux host 程序 ARM64 | `miniboard-ipd-linux-arm64.tar.gz` |
| Linux host 程序 ARMv7 32-bit | `miniboard-ipd-linux-arm32.tar.gz` |
| Windows 刷写器 | `MSU2.Flasher-windows-x64.exe` |
| Linux 刷写器 | `MSU2.Flasher-linux-x64` |
| macOS 刷写器 | `.dmg` |
