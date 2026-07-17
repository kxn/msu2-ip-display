# MSU2 刷写器设计规格

日期：2026-07-17

状态：开工前规格

## 目标

构建一个 Tauri 桌面程序，用于给当前 MSU2/CH32x035 USB 屏幕棒一键写入固定内置素材。

用户目标很窄：

1. 插入设备。
2. 程序自动检测设备并启用“写入”按钮。
3. 点击“写入”。
4. 看进度。
5. 写入完成后设备显示“未连接”。

首版不做素材包管理、不做自定义图片、不做编辑器、不做多设备批量刷写、不做 IP 显示保活。首版只解决“可靠刷入当前这套固定 UI 素材”。

## 明确不做

以下内容不进入首版：

- 素材包功能或素材包选择
- 默认界面显示 RGB565、页号、字库、Flash 布局等实现细节
- 默认界面显示设计说明、功能介绍、使用教程或营销文案
- 自定义素材导入
- 自动更新
- 后台托盘
- 主机 IP 获取和保活显示
- 多语言

调试和协议细节只允许出现在“复制记录”导出的详细记录中，不在默认界面显示。

## 推荐技术栈

采用 Tauri 2 + Rust 后端 + Web 前端。

后端：

- Rust
- `serialport-rs` 负责串口枚举、打开、读写
- Tauri command 负责前后端调用
- Tauri event 负责写入进度推送

前端：

- TypeScript
- 普通 HTML/CSS，暂不引入复杂 UI 框架
- 以静态布局和清晰状态为主

依据：

- Tauri command：<https://v2.tauri.app/develop/calling-rust/>
- Tauri crate：<https://docs.rs/tauri/latest/tauri/>
- Rust serialport：<https://docs.rs/serialport/latest/serialport/>
- USB VID/PID 信息：<https://docs.rs/serialport/latest/serialport/struct.UsbPortInfo.html>

## 已验证硬件事实

设备：

- USB VID/PID：`1A86:FE0C`
- Windows 串口示例：`COM4`
- 波特率：`19200`
- 串口参数：`8N1`
- 屏幕：`160x80`
- Flash 页大小：`256B`
- 单张彩色图：`160 * 80 * 2 = 25600B = 100` 页

握手：

- 发送：`00 4D 53 4E 43 4E`，即 `\0MSNCN`
- 正确回包：`00 4D 53 4E 43 4E`

图片格式：

- 内置素材必须是 RGB565 big-endian。
- 小端写入已验证会导致字体杂色和暗红横线。

## 固定写入布局

首版写入布局固定在代码中，不作为 UI 功能暴露：

| 资源 | 页范围 | 内容 |
| --- | --- | --- |
| 默认动图 | `0..3599` | 36 帧全部写入“未连接” |
| 获取中背景 | `3826..3925` | “获取 IP 中” |
| IP 背景 | `3926..4025` | 不带 IP 字符的背景 |
| 官方字库 | `4026+` | 保留，不写入 |

每张 `160x80` 彩色图占 100 页。默认动图帧起始页为 `0, 100, 200, ... 3500`。

## 内置素材

应用内置三份二进制资源：

- `offline.rgb565be`
- `acquiring.rgb565be`
- `ip_bg.rgb565be`

启动或写入前校验：

- 文件存在
- 每个文件长度为 `25600B`
- 固定布局不会覆盖 `4026+`

首版 UI 不显示素材名称、素材版本或素材细节。内部记录可包含素材版本，例如 `ui-c-green-v1`，用于排查。

## UI 规格

采用已确认的 v2 mock。

总览图：

![UI v2](C:/Users/kxn/Documents/Codex/2026-07-17/wox/outputs/msu2_flasher_mock_v2_contact_sheet.png)

单状态图：

- [未插入](C:/Users/kxn/Documents/Codex/2026-07-17/wox/outputs/msu2_flasher_mock_v2_no_device.png)
- [已连接](C:/Users/kxn/Documents/Codex/2026-07-17/wox/outputs/msu2_flasher_mock_v2_ready.png)
- [写入中](C:/Users/kxn/Documents/Codex/2026-07-17/wox/outputs/msu2_flasher_mock_v2_flashing.png)
- [完成](C:/Users/kxn/Documents/Codex/2026-07-17/wox/outputs/msu2_flasher_mock_v2_done.png)

默认界面只显示：

- 标题：`MSU2 刷写器`
- 顶部状态胶囊：`未检测到` / `设备已连接` / `写入中` / `已完成`
- 设备卡片：
  - `CH32x035`
  - 串口名，例如 `COM4`
  - `VID:PID`
  - `ID`
- 写入卡片：
  - 当前状态：`未连接` / `准备就绪` / `写入中` / `完成`
  - 主按钮：`写入` / `写入中` / `重新写入`
  - 进度条
- 底部一行记录：
  - 时间
  - 最近状态，例如 `设备就绪`、`写入中 64%`、`写入完成`

完成状态显示：

- 主状态：`完成`
- 按钮：`复制记录`
- 按钮：`重新写入`

默认界面禁止出现：

- `素材包`
- `RGB565`
- `160x80`
- 页号
- `Flash`
- `字库`
- `默认动图`
- `获取中背景`
- 设计解释或功能说明

这些信息只能进入隐藏详细记录。

## 交互状态

### 未插入

显示：

- 顶部：`未检测到`
- 设备：`未连接`
- 写入：`未连接`
- 按钮：`写入` 禁用
- 辅助按钮：`重新检测`

### 已连接

条件：

- 枚举到候选串口
- VID/PID 匹配或候选规则匹配
- 握手成功

显示：

- 顶部：`设备已连接`
- 设备：型号、端口、VID/PID、ID
- 写入：`准备就绪`
- 按钮：`写入` 启用

### 写入中

显示：

- 顶部：`写入中`
- 写入：`写入中`
- 百分比
- 进度条
- 按钮禁用

进度算法：

- 总任务单位：`36 + 1 + 1 = 38` 张图片
- 每张图 100 页
- 可按页级进度计算：总页数 `3800`
- 默认动图 3600 页，获取中 100 页，IP 背景 100 页

### 完成

条件：

- 所有页写入成功
- 预览命令执行完成
- 最终显示 page 0

显示：

- 顶部：`已完成`
- 写入：`完成`
- 进度条 100%
- `复制记录`
- `重新写入`

### 失败

mock 后续实现时补图。首版失败界面沿用同一布局：

- 顶部：`失败`
- 写入卡片标题：`无法写入`
- 最近记录显示一条可读错误，例如：
  - `COM4 被占用`
  - `设备已断开`
  - `写入失败`
- 主按钮：失败时禁用或变为 `重新检测`，按具体错误决定
- `复制记录` 可用

失败界面不展示协议包和页号，详细信息只在复制记录中。

## 自动检测

应用启动后每 1 秒枚举串口。

候选选择：

1. 优先匹配 USB VID/PID：`1A86:FE0C`
2. 如果系统未提供 VID/PID，则用产品名或描述包含 `CH32`、`CH32x035`、`WCH` 作为候选
3. 候选必须通过握手才进入 `Ready`

多设备策略：

- 首版只支持一个目标设备。
- 如果多个设备握手成功，选择第一个，同时在详细记录中写明“检测到多个设备”。
- 默认 UI 暂不提供设备下拉框，避免复杂化。

轮询规则：

- `Flashing` 状态下停止对当前端口的自动探测，避免干扰写入。
- 写入完成或失败后恢复检测。

## 后端模块

推荐结构：

```text
msu2-flasher/
  src/
    index.html
    main.ts
    styles.css
  src-tauri/
    src/
      main.rs
      device.rs
      protocol.rs
      flasher.rs
      assets.rs
      app_state.rs
      errors.rs
    assets/
      offline.rgb565be
      acquiring.rgb565be
      ip_bg.rgb565be
```

模块职责：

| 模块 | 职责 |
| --- | --- |
| `protocol.rs` | 构造握手、擦除、写页、显示图片命令；匹配回包 |
| `device.rs` | 枚举端口、识别候选、打开串口、握手 |
| `assets.rs` | 读取内置素材、校验长度、提供固定布局 |
| `flasher.rs` | 执行完整写入任务、重试、进度事件 |
| `app_state.rs` | 管理当前设备、任务状态、UI 可读状态 |
| `errors.rs` | 将内部错误转换为用户可读错误 |

## Tauri 命令

前端需要的 command：

```text
scan_devices() -> DeviceStatus
start_flash() -> FlashTaskStarted
copy_log() -> String
cancel_scan_lock_if_idle() -> ()
```

事件：

```text
device-status-changed
flash-progress
flash-finished
flash-failed
```

首版不提供取消写入。Flash 写入过程中中断风险高，按钮保持禁用。用户强行拔出设备时进入失败状态。

## 数据结构

设备状态：

```text
DeviceStatus {
  kind: NoDevice | Ready | Busy | ProtocolMismatch | Flashing | Done | Failed
  port_name: Option<String>
  vid_pid: Option<String>
  product: Option<String>
  serial: Option<String>
  message: String
}
```

刷写进度：

```text
FlashProgress {
  phase: Detect | Erase | Write | Preview | Done
  current_page: u32
  total_pages: u32
  percent: u8
  display_message: String
}
```

详细记录：

```text
LogEntry {
  timestamp: String
  level: Info | Warn | Error
  user_message: String
  detail: String
}
```

默认 UI 只显示 `user_message`。`detail` 只用于复制记录。

## 刷写流程

1. UI 调用 `start_flash()`
2. 后端锁定任务，防止重复点击
3. 打开当前 Ready 端口
4. 清空输入缓冲
5. 握手
6. 校验内置素材
7. 写默认动图：
   - 对 36 个起始页分别擦除 100 页
   - 对每帧写 100 页
8. 写获取中背景：
   - 起始页 `3826`
   - 写 100 页
9. 写 IP 背景：
   - 起始页 `3926`
   - 写 100 页
10. 预览：
    - 显示 page 0
    - 显示 page 3826
    - 显示 page 3926
    - 显示 page 0
11. 关闭串口
12. UI 进入完成状态

## 协议命令

握手：

```text
00 4D 53 4E 43 4E
```

擦除页：

```text
03 02 start_hi start_lo count_hi count_lo
```

写页：

```text
64 组:
  04 index data0 data1 data2 data3

尾包:
  03 03 page_hi24 page_mid page_low 01
```

显示图片页：

```text
SetXY:   02 00 x_hi x_lo y_hi y_lo
SetSize: 02 01 w_hi w_lo h_hi h_lo
Photo:   02 03 00 page_hi page_lo 00
```

## 重试与错误处理

重试：

- 擦除命令最多 3 次
- 写页命令最多 3 次
- 每次重试写详细记录
- 3 次失败后停止任务

常见错误映射：

| 内部错误 | 默认 UI 文案 | 详细记录 |
| --- | --- | --- |
| 未发现设备 | `未连接` | 枚举到的端口列表 |
| 串口打开失败 | `COM4 被占用` | OS 错误 |
| 握手失败 | `设备无响应` | 发送/接收字节 |
| 回包不匹配 | `写入失败` | 页号、期望回包、实际回包 |
| 写入中断开 | `设备已断开` | 最后成功页 |
| 素材校验失败 | `写入失败` | 文件名、长度 |

失败后：

- 停止写入
- 关闭串口
- UI 显示失败
- 允许复制记录
- 允许重新检测

## 记录策略

默认界面只显示最后一条简短记录。

`复制记录` 输出完整文本，包括：

- 应用版本
- 操作系统
- 端口信息
- VID/PID
- 设备 ID
- 素材版本
- 写入布局
- 每个阶段开始/完成时间
- 失败时的协议细节

复制记录用于排查，不用于默认展示。

## 视觉规范

按 v2 mock 实现：

- 浅灰背景
- 白色面板
- 小圆角，最大 8px
- 绿色表示正常/完成
- 黑绿色主按钮
- 灰色禁用按钮
- 页面不使用装饰性大图、渐变背景、营销区块
- 文案保持短句

默认窗口建议尺寸：

- `1040 x 620`
- 最小宽度 `860`
- 最小高度 `520`

响应规则：

- 小窗口下仍保持单列
- 不引入复杂导航
- 不隐藏主按钮

## 测试计划

单元测试：

- 握手包编码
- 擦除包编码
- 写页包编码
- 显示图片命令编码
- 回包匹配
- RGB565 big-endian 素材长度校验
- 固定布局不覆盖 `4026+`

集成测试：

- 枚举设备并按 VID/PID 识别
- 握手成功进入 Ready
- 串口占用进入 Busy
- 拔出设备进入 NoDevice 或 Disconnected
- 模拟回包失败触发 3 次重试
- 完整写入产生 100% 进度和完成事件

人工验收：

- Windows 上插入设备自动启用写入
- 写入按钮点击后进度变化
- 写入过程中按钮禁用
- 写入完成后屏幕显示“未连接”
- `复制记录` 可拿到详细日志
- 默认界面不显示素材包、页号、RGB565、字库等内部信息

## 发布范围

首版目标：

- Windows x64 便携版或安装包

后续平台：

- macOS
- Linux

原因：

- 当前设备和刷写流程都在 Windows 上验证过。
- Tauri/Rust 架构保持跨平台能力，但首个可用版本优先保证 Windows 质量。

## 开工边界

可以进入实现计划的条件：

- UI v2 已确认
- 首版不做素材包功能已确认
- 固定素材和固定写入布局已确认
- 默认界面不展示实现细节已确认

实现计划应从以下任务开始：

1. 创建 Tauri 项目骨架
2. 移植协议编码和串口写入逻辑到 Rust
3. 内置三份素材并校验
4. 实现设备检测和握手
5. 实现 v2 UI
6. 连接进度事件
7. 做 Windows 实机刷写验证
