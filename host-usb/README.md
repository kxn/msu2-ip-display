# Host USB

`miniboard-ipd` 是运行在 Linux 无头设备上的 host 端守护进程。它检测 MSU2 MINI USB 小屏，拿到可显示的 IPv4 后把地址显示到屏幕上。

设计文档：

- `docs/superpowers/specs/2026-07-17-host-usb-ip-display-design.md`
- `docs/superpowers/specs/2026-07-18-host-usb-qr-display-design.md`

## 常用命令

```bash
miniboard-ipd run
miniboard-ipd run --interface eth0
miniboard-ipd run --unflashed
miniboard-ipd run --debug
miniboard-ipd run --show qr
miniboard-ipd run --show 'qr:http://{ip}/'
miniboard-ipd install --interface eth0 --dhcp-fail-delay-seconds 45
miniboard-ipd uninstall
miniboard-ipd status
miniboard-ipd --help
```

`--show` 控制拿到 IP 后的显示方式：

- 不写 `--show`：默认显示两行文字 IPv4。
- `--show ip`：显式使用文字 IPv4。
- `--show qr`：显示默认二维码，内容为 `http://{ip}/`。
- `--show 'qr:http://{ip}:8080/'`：用模板生成二维码，`{ip}` 会替换成实际 IPv4。

二维码模式会在启动时用 `255.255.255.255` 做最坏情况校验。模板太长时程序会直接报错，不启动服务。

## 一行安装

安装脚本会下载当前 Linux 架构匹配的 GitHub Release 产物，校验 SHA-256，把 `miniboard-ipd` 安装到 `/usr/local/bin`，然后执行 `miniboard-ipd install ...` 注册服务。
`install` 会写入服务文件，并在 init 系统支持时启用开机启动，但不会立刻启动或重启服务。这样在 chroot 里准备嵌入式 rootfs 时不会因为启动服务失败而中断；进入目标系统后按输出提示手动启动服务。

```bash
curl -fsSL https://raw.githubusercontent.com/kxn/msu2-ip-display/master/scripts/install-miniboard-ipd.sh | sudo sh
```

未提前用 flasher 刷入本项目资源的板子，可以安装时启用 `--unflashed`：

```bash
curl -fsSL https://raw.githubusercontent.com/kxn/msu2-ip-display/master/scripts/install-miniboard-ipd.sh | sudo sh -s -- --unflashed
```

这个模式不会改写小屏 flash。host 端还没连上时，小屏保持出厂默认状态；host 连上后，获取 IP 中和 DHCP 失败状态由 host 直接写屏。文字 IP 使用官方保留页里的数字 glyph，并在运行时画边框，所以状态切换会比刷过资源的默认模式慢一点。

传给 `sh -s --` 后面的参数会固化进生成的 service/init 脚本：

```bash
curl -fsSL https://raw.githubusercontent.com/kxn/msu2-ip-display/master/scripts/install-miniboard-ipd.sh | sudo sh -s -- --interface eth0 --dhcp-fail-delay-seconds 45
```

安装为二维码显示模式：

```bash
curl -fsSL https://raw.githubusercontent.com/kxn/msu2-ip-display/master/scripts/install-miniboard-ipd.sh | sudo sh -s -- --show qr
```

使用自定义二维码模板：

```bash
curl -fsSL https://raw.githubusercontent.com/kxn/msu2-ip-display/master/scripts/install-miniboard-ipd.sh | sudo sh -s -- --show 'qr:http://{ip}:8080/'
```

排查问题时可以启用详细日志：

```bash
curl -fsSL https://raw.githubusercontent.com/kxn/msu2-ip-display/master/scripts/install-miniboard-ipd.sh | sudo sh -s -- --debug
```

OpenRC 服务会把 stdout/stderr 写到 `/var/log/miniboard-ipd.log`。

只安装 binary，不注册服务：

```bash
curl -fsSL https://raw.githubusercontent.com/kxn/msu2-ip-display/master/scripts/install-miniboard-ipd.sh | sudo sh -s -- --no-service
```

## 构建

```bash
cargo test
cargo build --release
```

`install` 会把命令行选项，包括 `--debug`、`--show` 和 `--unflashed`，写入生成的 service/init 启动命令。v1 不读取配置文件。
