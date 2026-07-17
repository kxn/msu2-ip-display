# Miniboard

MSU2 MINI / MSU2 系列 USB 小屏调研与刷写工具仓库。

## 目录

- `flasher/`: 当前可用的 MSU2 MINI Tauri/Rust 刷写器。
- `host-usb/`: 未来 host-side USB/串口通信程序预留目录。
- `docs/`: 仓库内整理后的文档。
- `references/`: 去重后的官方资料、逆向资料、验证日志和生成资产。

## 常用命令

```powershell
cd flasher
npm run build
```

```powershell
cd flasher/src-tauri
cargo test
```

```powershell
cd host-usb
cargo test
```

## 当前结论

当前 flasher 是局部固定资产刷写器，不是完整 1MB 出厂 Flash 恢复工具。协议和布局请先看 `docs/msu2-protocol-and-flash-layout.md`。
