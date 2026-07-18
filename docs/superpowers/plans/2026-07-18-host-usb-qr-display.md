# Host USB 二维码显示模式实施计划

目标：给 `miniboard-ipd` 增加一个 `--show` 选项，使 host 端在拿到 IPv4 后可以显示现有文字 IP，或显示由当前 IPv4 生成的 URL 二维码。

总体结构：`cli.rs` 把 `--show` 解析成 `DisplayMode` 并保存到 `RunOptions`；新增 `qr_display.rs` 负责模板校验和整屏 RGB565BE 二维码图生成；`display.rs` 把二维码图拆成 MSU2 LCD 直写命令；`runtime.rs` 在 `ShowIp` 时根据模式选择文字 IP 或二维码，并为二维码使用白色 keepalive 像素。

约束：

- host 程序仍然是无头 Linux 程序，尽量少依赖。
- 不新增配置文件；`install` 继续把命令行参数固化进 service/init 脚本。
- 用户可见显示配置只保留一个参数：`--show`。
- 默认模式是文字 IP。
- 二维码默认模板是 `http://{ip}/`。
- 二维码模板必须包含 `{ip}`。
- 二维码模式使用白底黑码。
- 二维码使用 M 纠错、每 module 固定 `2x2` 像素、标准 4 module quiet zone，QR version 必须小于等于 3。
- 二维码模式在 IP 内容变化时写满 `160x80` RGB565BE 全屏图，keepalive 使用白色像素。
- 文字模式保持现有两行数码管字体 IP 和黑色 keepalive 像素。

## 任务 1：CLI 显示模式

文件：

- 修改 `host-usb/src/cli.rs`

接口：

- 新增 `pub enum DisplayMode { Text, Qr { template: String } }`
- `RunOptions` 增加 `show`
- `RunOptions::service_args()` 在非默认文字模式时写入 `--show`

步骤：

- 增加默认 `--show ip`、`--show qr`、`--show qr:<template>`、非法值拒绝等 CLI 测试。
- 先运行测试确认失败。
- 实现 `DisplayMode`、默认值和解析逻辑。
- 重新运行 CLI 测试确认通过。

## 任务 2：二维码模板校验和渲染

文件：

- 新建 `host-usb/src/qr_display.rs`
- 修改 `host-usb/src/lib.rs`
- 修改 `host-usb/Cargo.toml`
- 修改 `host-usb/Cargo.lock`

接口：

- `pub const QR_WHITE: u16 = 0xffff`
- `pub fn validate_template(template: &str) -> Result<(), String>`
- `pub fn render_qr_rgb565be(template: &str, ip: Ipv4Addr) -> Result<Vec<u8>, String>`

步骤：

- 增加 `qrcodegen = "1"`。
- 增加模板必须包含 `{ip}`、过长模板拒绝、整屏白底且存在黑色 module 的测试。
- 先运行测试确认失败。
- 用 `qrcodegen::{QrCode, QrCodeEcc}` 实现 QR 生成；用 `255.255.255.255` 做最坏情况启动校验。
- 固定 `scale = 2`、`border = 4`、`version <= 3`。
- 输出完整 `160*80*2` RGB565BE 字节图。
- 重新运行二维码模块测试确认通过。

## 任务 3：DisplayRenderer 二维码写屏命令

文件：

- 修改 `host-usb/src/display.rs`

接口：

- `DisplayRenderer::qr(ip: Ipv4Addr, template: &str) -> Result<Vec<WireWrite>, String>`
- `DisplayRenderer::keepalive_white() -> Vec<WireWrite>`

步骤：

- 增加整屏 LCD 写入区域和白色 keepalive 的测试。
- 先运行测试确认失败。
- 抽出 `lcd_region_writes(x, y, width, height, bytes)`，把 RGB565BE 字节图拆成 `set_xy`、`set_size`、`load_lcd_address` 和多包 `write_lcd_data`。
- 二维码整屏写入预期是 103 个 `WireWrite`。
- 重新运行 display 相关测试确认通过。

## 任务 4：runtime 接线

文件：

- 修改 `host-usb/src/runtime.rs`
- 修改 `host-usb/src/daemon.rs`

接口：

- runtime 保存 `RunOptions.show`。
- `DaemonAction::ShowIp(ip)` 在文字模式下走 `DisplayRenderer::ip`，在二维码模式下走 `DisplayRenderer::qr`。
- runtime 记录当前 keepalive 像素颜色：文字和状态页用黑色，二维码用白色。

步骤：

- 扩展 fake I/O 写入分类，能识别二维码整屏写入和白色 keepalive。
- 增加“二维码模式不发送文字 IP”和“二维码模式 keepalive 使用白色像素”的测试。
- 先运行测试确认失败。
- 实现 runtime 模式分发和 keepalive 颜色状态。
- 重新运行 runtime 相关测试确认通过。

## 任务 5：版本、验证和发布

文件：

- 修改 `host-usb/Cargo.toml`
- 修改 `host-usb/Cargo.lock`
- 修改 `flasher/package.json`
- 修改 `flasher/package-lock.json`
- 修改 `flasher/src-tauri/Cargo.toml`
- 修改 `flasher/src-tauri/Cargo.lock`
- 修改 `flasher/src-tauri/tauri.conf.json`

步骤：

- 项目版本从 `0.1.9` bump 到 `0.1.10`，不误改第三方依赖版本。
- 运行完整验证：

```bash
cargo fmt --manifest-path host-usb/Cargo.toml -- --check
cargo test --manifest-path host-usb/Cargo.toml -- --nocapture
sh scripts/test-install-miniboard-ipd.sh
cargo fmt --manifest-path flasher/src-tauri/Cargo.toml -- --check
cargo test --manifest-path flasher/src-tauri/Cargo.toml -- --nocapture
npm run build
git diff --check
```

- 提交并推送到 `master`。
- 打 tag `v0.1.10` 并推送。
- 观察 GitHub Actions 的 CI 和 Release workflow，确认 release 产物生成。

## 自检

- CLI、模板校验、二维码渲染、写屏命令、runtime 分发、keepalive、测试和发版路径都有覆盖。
- v1 没有新增配置文件，也没有把二维码静态资源写进 flasher。
- 用户使用示例：`miniboard-ipd run --show qr` 或 `miniboard-ipd install --show 'qr:http://{ip}/'`。
