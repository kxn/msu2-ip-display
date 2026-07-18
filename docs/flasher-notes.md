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

1. 每秒枚举串口，优先匹配 VID/PID `1A86:FE0C`。
2. `NoDevice` 状态只做枚举；检测到目标设备后打开串口、握手、整屏显示 `等待写入`。
3. `Ready` 状态由后端 display worker 持有串口，每 `800ms` 写一个 `1x1` 黑色像素作为 keepalive，防止固件回到离线动图。
4. `Ready` 或 `Done` 状态下，如果同一个端口仍在枚举列表中，扫描只返回后端状态快照，不再重复打开串口握手。
5. 点击写入时先停止 display worker 并等待串口释放，再进入 `Flashing` 状态，校验内置资产长度并写入固定页面。
6. 写入 8 个资源：两帧离线动图、离线静图、获取中状态图、DHCP 失败状态图、IP 背景、启动 logo、资源目录页，共 `607` 页。
7. 写入完成后进入 `Done` 状态，整屏显示 `写入完成` 并继续用 `1x1` 像素 keepalive 保持画面；正常流程不再自动预览 page `0`。
8. 写入失败进入 `Error` 状态，PC UI 显示错误；下一次扫描如果设备仍在，会重新显示 `等待写入` 并恢复 `Ready`。

离线动图不再来自 36 帧合并资源。当前 flasher 只写两张全屏 RGB565BE 图：page `0..99` 为可见的“未连接”，page `100..199` 为空背景；同时写 page `4094` 的资源目录，把 E0 设置为 `interval=900ms`、`count=2`、`start=0`，让固件在两帧之间切换。离线静图由 E1 指向 page `200..299`，显示同一张不闪烁的“未连接”图。

当前紧凑 Flash 资源布局：

| 用途 | 页 |
| --- | ---: |
| 离线动图可见帧 | `0..99` |
| 离线动图空白帧 | `100..199` |
| 离线静图 | `200..299` |
| 获取 IP 中 | `300..399` |
| DHCP 失败 | `400..499` |
| IP 背景 | `500..599` |
| 启动 logo | `3820..3825` |
| 资源目录 | `4094` |

flasher 不再写入 `flasher/src-tauri/assets/offline_animation.rgb565be`，也不再擦写 `3726..3825`，避免覆盖 ASCII 字库尾部和启动 logo。

自动扫描每秒触发一次。扫描本身只负责发现目标设备是否存在；屏幕显示和 keepalive 由后端会话状态机统一管理。`Ready` 和 `Done` 会持续持有串口发送微小 keepalive，`Flashing` 独占串口执行擦除和写入，因此 keepalive 不会和 flash 命令交错。

## 实机验证证据

`references/artifacts/logs/task8-hardware-verify.log` 显示：

- 扫描到 `COM4`
- VID/PID 为 `1A86:FE0C`
- 握手成功
- `images=38 pages=3800`
- 进度到 `page=3800/3800`
- 输出 `DONE`

上述日志来自旧布局实机验证；当前紧凑写入计划为 `assets=8 pages=607`。

`references/artifacts/logs/task8-page0-preview-verify.log` 显示最后 page 0 预览命令：

```text
show_page0_tx=02 03 00 00 00 00
show_page0_rx=02 03 00 00 00 00
```

## 限制

- 只写固定内置资产，不导入自定义素材。
- 不是完整 1MB Flash 恢复工具。
- 串口参数当前固定为 `921600 8N1`，启用 RTS/CTS 硬件流控；这是对照官方 1.47/2.8 参考代码后在 MINI 上重新验证过的高速路径。
- 不实现官方 Demo 的完整 host-side API。
- 写入时不支持取消，避免中途打断导致 Flash 状态不完整。

## 屏幕端写入状态

新增的写入状态显示使用官方 Demo 里的直接 LCD 区域写入能力：

- `LCD_ADD`：设置区域后发送 `02 03 07 00 00 00`，实测会回显同一个 6 字节包。
- `LCD_DATA`：发送 `04 index data0 data1 data2 data3` 数据组后用 `02 03 08 size_hi size_lo 00` 提交，实测不回包。
- 直接 LCD 写入跟随官方参考代码：`LCD_Set_XY` 和 `LCD_Set_Size` 后不等待回包，只等待 `LCD_ADD` 确认，避免每个局部刷新多空等两次。
- 因此屏幕状态是 best-effort：探测或写入失败会自动禁用屏幕端进度，但不会中断 flash 写入。

状态图使用一张完整 `160x80` 初始 RGB565 图，保证开始刷写时先覆盖旧画面，避免和设备原有屏幕内容混在一起。刷写过程中只局部更新百分比小块和进度条新增填充区域；百分比按 `1%` 粒度变化，同一个百分比不会重复刷新。

`等待写入` 和 `写入完成` 也是直接 LCD 整屏写入资产，不占用 Flash 页。整屏图只在状态变化时写一次；保持显示时只更新右下角 `159,79` 的一个黑色像素，避免慢速屏幕做无意义的大面积刷新。

## 后续方向

- `host-usb/` 可独立做完整通信层，支持更多官方 API。
- host-side 程序应支持不同型号/固件的串口参数差异。
- 如果要做完整 Flash 备份/恢复，应基于官方 1MB 固件和实机读写再设计，不要复用当前局部写入逻辑冒充全量恢复。
