# Host USB QR 显示模式设计

## 状态

本文记录 `miniboard-ipd` 新增二维码显示模式的设计。该功能基于当前已经可用的文字 IP 显示链路扩展，不改 flasher 资源布局，不增加配置文件。

## 目标

在无头 Linux 设备拿到 IPv4 后，除了现有两行文字 IP 外，可以把一个包含 IP 的 URL 生成二维码显示在 MSU2 MINI 屏幕上，便于用户直接用手机扫码打开设备页面。

二维码模式优先保证现场扫码成功率。屏幕只有 `160x80`，因此 v1 不追求塞入很长 URL，也不显示额外边框、标题或装饰。

## 非目标

- 不支持 IPv6。
- 不支持多个二维码或轮播。
- 不在 flasher 中新增二维码静态资源。
- 不引入配置文件。
- 不提供运行时动态修改模板的 IPC。
- 不支持 1px/module 的高密度二维码作为正式模式。

## 命令行配置

只新增一个用户可见配置项：`--show`。

```text
miniboard-ipd run --show ip
miniboard-ipd run --show qr
miniboard-ipd run --show qr:http://{ip}/
miniboard-ipd install --show qr:http://{ip}:8080/
```

规则：

- 不写 `--show` 时默认等价于 `--show ip`。
- `--show ip` 使用现有两行文字 IP 显示。
- `--show qr` 使用默认模板 `http://{ip}/`。
- `--show qr:<template>` 使用模板生成二维码，模板中的 `{ip}` 替换为实际 IPv4。
- `install` 继续把命令行参数固化进 service/init 脚本。

模板必须包含 `{ip}`。这样可以避免用户误写固定 URL，导致设备 IP 变化后屏幕内容不对应。

## 二维码内容和校验

启动时如果进入二维码模式，程序用最坏 IPv4 `255.255.255.255` 替换模板并生成二维码，提前校验可显示性。校验失败时 `run` 和 `install` 都应返回错误，不启动服务。

v1 校验规则：

- QR 纠错等级使用 `M`。
- 每个 module 固定 `2x2` 像素。
- quiet zone 使用标准 4 modules。
- QR version 必须小于等于 3。
- 最终二维码图块必须能放进 `80x80` 高度内；标准布局下 version 3 为 `74x74`。

这些规则来自 2026-07-18 实机测试：`66px` 和 `74px` 白底黑码均可扫描，`74px` 已经接近实际可用上限，扫码稳定性会受手机相机和扫码软件影响。正式模式应把 version 3 作为上限，不为更长 URL 牺牲稳定性。

## 屏幕渲染

二维码模式拿到可显示 IPv4 后，不显示文字 IP，也不使用 `IP_BACKGROUND_PAGE`。程序运行时生成一张完整 `160x80` RGB565BE 图，通过现有 direct LCD write 协议写满屏幕。

画面规则：

- 白底黑码。
- 二维码居中。
- 不画边框。
- 不显示 IP 文本、标题、端口提示或状态文字。
- 保留二维码自身 quiet zone。

颜色选择以扫码成功率为准。实测中白底黑码更符合手机默认扫码算法预期，因此 v1 固定使用白底黑码。

## 协议路径

复用当前已验证的 LCD 区域写入命令：

1. `set_xy(0, 0)`
2. `set_size(160, 80)`
3. `load_lcd_address`
4. 按 256 字节分块发送 `write_lcd_data`

二维码只在 IP 显示内容变化时重写全屏。没有 IP、DHCP 失败、设备拔插等状态继续使用现有页面显示路径。

## Keepalive

二维码显示后继续使用现有 keepalive 机制。keepalive 像素应写在 `x=159,y=79`，颜色使用白色 `0xffff`，使其落在白底区域中，不影响扫码。

文字 IP 模式仍使用现有黑色 keepalive 像素。

## 代码结构

建议新增或调整：

- `host-usb/src/cli.rs`
  - 增加 `DisplayMode`：`Text` / `Qr { template }`。
  - 解析 `--show`。
  - `service_args()` 固化 `--show`。
  - 在解析阶段校验二维码模板。
- `host-usb/src/qr_display.rs`
  - 负责模板替换、QR 生成、尺寸校验、RGB565BE 图生成。
  - 暴露 `QrTemplate` 和 `QrRenderer`。
- `host-usb/src/display.rs`
  - 增加 `DisplayRenderer::qr(ip, template)`，返回全屏 direct LCD write 命令。
  - keepalive 根据显示模式或最近显示内容选择白/黑像素。
- `host-usb/src/runtime.rs`
  - 对 `DaemonAction::ShowIp(ip)` 按 `RunOptions.show` 选择文字 IP 或二维码。

## 依赖选择

二维码生成建议使用小型纯 Rust 库 `qrcodegen`。理由：

- QR 编码容易写错，手写不划算。
- 该库不需要图像运行时，不引入 GUI 或系统动态库。
- 最终输出只需读取 module 布尔值并写入 RGB565BE 缓冲区。

不引入 `image`、`qrcode`、`Pillow` 或 Python runtime 到正式 host 程序。

## 测试计划

单元测试：

- `--show` 默认值为 `ip`。
- `--show qr` 使用默认模板。
- `--show qr:<template>` 保留模板并写入 service 参数。
- 不含 `{ip}` 的模板被拒绝。
- 超过 version 3 的模板被拒绝。
- 二维码渲染输出为 `160*80*2` 字节。
- 二维码画面角落为白色，存在黑色 module，居中位置稳定。
- runtime 在 QR 模式下显示 IP 时发送 direct LCD write，而不是文字 IP RAM mix。
- QR 模式 keepalive 使用白色像素。

实机测试：

- 显示 `http://255.255.255.255/`。
- 显示接近 version 3 上限的 URL。
- 使用手机相机和微信扫码确认可识别。
- 长时间显示二维码，确认 keepalive 不让固件回到离线动画。

## 错误处理

- `--show` 值不是 `ip`、`qr` 或 `qr:<template>` 时返回 CLI 错误。
- QR 模板不含 `{ip}` 时返回 CLI 错误。
- 模板替换最坏 IP 后超过 v1 上限时返回 CLI 错误，错误中说明 URL 太长。
- 运行中生成二维码理论上不应失败；如果失败，记录 warning 并保持 pending/当前屏幕状态，避免 daemon 崩溃。

## 发布

实现完成后 bump host 和 flasher 版本，发布新 tag。安装脚本继续使用 latest release，因此目标设备可通过重复执行 curl 安装命令升级。
