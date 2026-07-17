# MSU2 Protocol And Flash Layout

本文整理当前 flasher、官方 Python Demo、官方 PDF 和实机验证日志里的有用信息。

## 信息来源

- 实机验证日志：`references/artifacts/logs/task8-hardware-verify.log`
- Page 0 预览验证：`references/artifacts/logs/task8-page0-preview-verify.log`
- 当前 flasher 协议实现：`flasher/src-tauri/src/protocol.rs`
- 当前 flasher 串口实现：`flasher/src-tauri/src/device.rs`
- 官方 MSU2/MINI 指南：`references/vendor/docs/使用及开发指南-MSU2系列可编程USB屏幕_20241102224357.pdf`
- 官方 1.54 寸 Demo：`references/vendor/msu2-programmable-usb-screen/MSU2 演示Demo及固件V1.0/Python源码/MSU2_DemoV1.0.py`
- 官方 1.47/2.8 寸参考代码：`references/vendor/source/msu2-lite-pro/`

## 已实机确认的 MINI flasher 参数

- USB VID/PID: `1A86:FE0C`
- Windows 端口样例：`COM4`
- 串口：`921600 8N1`，RTS/CTS 硬件流控
- 握手：`00 4D 53 4E 43 4E`，即 `\0MSNCN`
- LCD: `160x80`
- 彩色图格式：RGB565 big-endian
- 单张 `160x80` 彩色图大小：`160 * 80 * 2 = 25600` bytes
- Flash 页大小：`256` bytes
- 单张 `160x80` 彩色图占 `100` 页

之前的硬件验证在 `COM4` 上完成了 `3800/3800` 页写入，并在最后发送 page 0 预览命令，回包匹配。

## 官方资料修正点

早期逆向验证曾使用 `19200 8N1` 无流控，设备也能握手；但对照 `data` 中后续拿到的官方 1.47 寸和 2.8 寸 Python 参考代码后，已在当前 MINI 上重新确认 `921600 8N1 + RTS/CTS` 可以握手并工作。当前 flasher 因此使用高速路径。

官方参考代码实际打开串口时使用：

```text
921600 baud, xonxoff=False, rtscts=True
```

所以未来 `host-usb/` 不要把 `19200` 写死成全系列唯一参数。建议至少把串口参数做成设备配置，或在后续实机测试后做自动探测/协商。

## 握手流程

官方 Demo 会等待设备周期性输出类似 `00 4D 53 4E 30 31` 的 `\0MSN01` 版本广播，再由上位机发送：

```text
00 4D 53 4E 43 4E
```

设备确认连接时会回：

```text
00 4D 53 4E 43 4E
```

当前 flasher 的握手逻辑允许回包缓冲里夹带设备广播，只要包含 `\0MSNCN` 即认为成功。

## Flash 命令

Flash 总容量按官方文档为 `1024KB`，即 `4096` 页，每页 `256` bytes。

### 擦除页

```text
03 02 start_hi start_lo count_hi count_lo
```

含义：

- `03`: Flash 操作
- `02`: 按页擦除
- `start`: 起始页，16-bit big-endian
- `count`: 页数，16-bit big-endian

示例：擦除 `3826` 开始的 `100` 页：

```text
03 02 0E F2 00 64
```

### 写入页缓冲

一页 256 bytes 先拆成 64 组，每组写 4 bytes：

```text
04 index data0 data1 data2 data3
```

`index` 范围是 `00..3F`。

### 提交一页写入

当前 flasher 使用快速写入形式，64 组 `04` 包后接：

```text
03 03 page_hi page_mid page_lo 01
```

含义：

- `03`: Flash 操作
- `03`: 快速写 Flash
- `page`: 24-bit big-endian 页号
- `01`: 写 1 页

官方旧 Demo 里还有普通写入形式：

```text
03 01 page_hi page_mid page_lo page_count
```

当前 flasher 没有使用普通写入。

### 读 Flash 字节

官方 Demo 的读字节命令：

```text
03 00 addr_hi addr_mid addr_low 00
```

当前 flasher 不需要读 Flash。

## LCD 显示命令

设置显示起点：

```text
02 00 x_hi x_lo y_hi y_lo
```

设置显示尺寸：

```text
02 01 width_hi width_lo height_hi height_lo
```

显示彩色图片页：

```text
02 03 00 page_hi page_lo 00
```

当前 flasher 预览流程会依次显示 page `0`、`3826`、`3926`，最后回到 page `0`。

官方 Demo 还包含单色图片显示、彩色背景叠加、直接写 LCD 数据、设置前景/背景色等接口。后续 host-side 程序如果要做完整上位机能力，应从官方 Python API 中继续整理这些命令。

## 当前 MINI 固定写入布局

当前 flasher 只做局部固定资产写入：

| 页范围 | 页数 | 内容 |
| --- | ---: | --- |
| `0..3599` | `3600` | 36 帧待机动图，每帧 100 页；当前写入同一张离线图 |
| `3726..3825` | `100` | DHCP 失败状态图 |
| `3826..3925` | `100` | 获取中背景 |
| `3926..4025` | `100` | IP 背景 |
| `4026+` | 保留 | 官方字库/单色图/其他资源，不写入 |

官方指南说明 MINI 离线状态会循环播放 36 张彩色图，每 100ms 切一帧，合计 3.6 秒。当前 flasher 写 36 帧同一张离线图，是为了稳定恢复离线提示画面，不是恢复官方出厂动画。

`3726..3825` 是后续 host-side IP 显示程序新增的 `DHCP失败` 状态图页。它位于当前固定布局中未写入的 `3600..3825` 空档内；官方资料曾在这一带放过大号数码管、ASCII 字库或 Logo 示例资源。当前已实机采用 `4026+` 的 `N24X33P` 24x33 数码管显示 IP，因此固定 flasher 优先保留 `4026+`。

## 官方/参考布局线索

官方 1.47/2.8 参考代码注释中出现了这些地址线索：

| 起始页 | 资源线索 |
| ---: | --- |
| `0`、`100`、...、`3500` | 36 张 `160x80` 彩色动画帧 |
| `3600` | 单色 Demo 图示例 |
| `3629` | 数码管图像示例 |
| `3651` | ASCII 字库示例 |
| `3726` | 当前自定义 DHCP 失败状态图 |
| `3820` | MINI logo 类资源 |
| `3826` | `160x80` 彩色背景 |
| `3926` | `160x80` 彩色相册/背景 |
| `4026` | 小字库/数字类资源 |
| `4038` | 单色图类资源 |

这些地址来自官方参考代码和文档注释。实际不同屏幕型号、固件版本、京东方屏版本可能会有差异，后续做完整 host-side 工具时应以固件版本和实机读取结果再确认。

## 当前不确定项

- `921600 + RTS/CTS` 已在当前 MINI 上确认可握手；其他型号、旧固件或不同屏幕版本是否都适用，仍需按设备确认。
- 官方 `Flash_MINI_V1.1(京东方屏幕).bin` 与 `低速切换` 版本只在文件尾部少量字节不同，具体语义未解析。
- 当前内置三张 `rgb565be` 资产没有在官方 1MB 固件里找到完全相同片段，说明它们是自定义生成资产，不是官方出厂图。
- 完整上位机 API 还包括 SFR 读写、ADC、RGB LED、截图投屏等能力，当前文档只覆盖 flasher 和后续 host 通信最需要的核心部分。
