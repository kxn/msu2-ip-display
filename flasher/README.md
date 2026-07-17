# MSU2 Flasher

MSU2 MINI 固定资产刷写器。

## 使用

1. 插入 MSU2 MINI。
2. 等待应用显示设备。
3. 点击 `写入`。
4. 等待进度完成。

## 开发

```powershell
npm run build
```

```powershell
cd src-tauri
cargo test
```

## 注意

- 写入前关闭其他串口工具。
- 失败时点击 `复制记录`，保留日志用于排查。
- 这是局部固定资产刷写器，不是完整 1MB Flash 恢复工具。
