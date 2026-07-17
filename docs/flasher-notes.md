# Flasher Notes

当前 flasher 是一个 Windows 上验证过的 Tauri 2 + Vite + Rust 桌面程序，用于给 MSU2 MINI 写入固定内置画面。

## 位置

- 前端：`flasher/src/`
- Rust 后端：`flasher/src-tauri/src/`
- 内置资产：`flasher/src-tauri/assets/`
- Tauri 配置：`flasher/src-tauri/tauri.conf.json`

## 构建和测试

```powershell
cd flasher
npm run build
```

```powershell
cd flasher/src-tauri
cargo test
```

## 当前行为

1. 枚举串口，优先匹配 VID/PID `1A86:FE0C`。
2. 打开候选串口并发送 `\0MSNCN` 握手。
3. 握手成功后 UI 进入可写状态。
4. 点击写入后校验三张内置资产长度。
5. 写入 36 帧离线图、获取中背景、IP 背景，共 `3800` 页。
6. 写入完成后发送预览命令，最终显示 page `0`。

## 实机验证证据

`references/artifacts/logs/task8-hardware-verify.log` 显示：

- 扫描到 `COM4`
- VID/PID 为 `1A86:FE0C`
- 握手成功
- `images=38 pages=3800`
- 进度到 `page=3800/3800`
- 输出 `DONE`

`references/artifacts/logs/task8-page0-preview-verify.log` 显示最后 page 0 预览命令：

```text
show_page0_tx=02 03 00 00 00 00
show_page0_rx=02 03 00 00 00 00
```

## 限制

- 只写固定内置资产，不导入自定义素材。
- 不是完整 1MB Flash 恢复工具。
- 串口参数当前固定为 `19200 8N1`，适用于已验证的 MINI flasher 路径。
- 不实现官方 Demo 的完整 host-side API。
- 写入时不支持取消，避免中途打断导致 Flash 状态不完整。

## 屏幕端写入状态

新增的写入状态显示使用官方 Demo 里的直接 LCD 区域写入能力：

- `LCD_ADD`：设置区域后发送 `02 03 07 00 00 00`，实测会回显同一个 6 字节包。
- `LCD_DATA`：发送 `04 index data0 data1 data2 data3` 数据组后用 `02 03 08 size_hi size_lo 00` 提交，实测不回包。
- 因此屏幕状态是 best-effort：探测或写入失败会自动禁用屏幕端进度，但不会中断 flash 写入。

状态图中文字和百分比来自 `flasher/src-tauri/assets/flash_status_*.rgb565be` 小块素材，进度条填充由 Rust 生成纯色 RGB565 区域。进度只按 `25/50/75/100` 四个粗粒度阶段更新，避免慢速串口上频繁刷新。

## 后续方向

- `host-usb/` 可独立做完整通信层，支持更多官方 API。
- host-side 程序应支持不同型号/固件的串口参数差异。
- 如果要做完整 Flash 备份/恢复，应基于官方 1MB 固件和实机读写再设计，不要复用当前局部写入逻辑冒充全量恢复。
