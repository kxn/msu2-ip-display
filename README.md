# MSU2 IP Display

把一块 MSU2 MINI USB 小屏变成无头 Linux 设备的 IP 显示器。

设备部署到现场后，经常是 DHCP 自动拿地址，但旁边没有显示器。这个项目提供两件配套工具：先用桌面刷写器把小屏刷成适合显示 IP 的资源布局，再在 Linux 设备上安装 `miniboard-ipd`，插上小屏后自动显示当前 IPv4，或者显示可扫码打开的二维码。

<p align="center">
  <img src="docs/images/ip-text-display.jpg" alt="MSU2 MINI 显示文字 IPv4" width="32%">
  <img src="docs/images/qr-display.jpg" alt="MSU2 MINI 显示二维码" width="32%">
  <img src="docs/images/acquiring-ip.jpg" alt="MSU2 MINI 显示获取 IP 中" width="32%">
</p>

## 准备硬件

需要一块 MSU2 MINI USB 小屏。购买地址：

<https://item.taobao.com/item.htm?id=841679900812>

购买时选已经组装好的版本，拿到后直接用 USB 连接电脑或 Linux 设备即可。

## 快速开始

### 1. 刷写小屏资源

从 [Latest Release](https://github.com/kxn/msu2-ip-display/releases/latest) 下载 `MSU2 Flasher`：

| 系统 | 文件 |
| --- | --- |
| Windows x64 | `MSU2.Flasher-windows-x64.exe` |
| Linux x64 | `MSU2.Flasher-linux-x64` |
| macOS Intel | `MSU2.Flasher_<版本>_x64.dmg` |
| macOS Apple Silicon | `MSU2.Flasher_<版本>_aarch64.dmg` |

打开刷写器，插入 MSU2 MINI，等待应用识别设备后点击 `写入`。刷写完成后，小屏就具备待机动画、获取 IP、DHCP 失败、文字 IP 和二维码显示需要的资源。

Linux 版 flasher 下载后需要加执行权限：

```sh
chmod +x ./MSU2.Flasher-linux-x64
./MSU2.Flasher-linux-x64
```

### 2. 在 Linux 设备上安装 IP 显示服务

默认安装后显示文字 IPv4：

```sh
curl -fsSL https://raw.githubusercontent.com/kxn/msu2-ip-display/master/scripts/install-miniboard-ipd.sh | sudo sh
```

安装后插入已经刷好的 MSU2 MINI。设备拿到 IP 后，屏幕会直接显示 IPv4。

## 显示二维码

二维码模式适合设备有 Web 管理页面的场景。默认二维码内容是 `http://{ip}/`：

```sh
curl -fsSL https://raw.githubusercontent.com/kxn/msu2-ip-display/master/scripts/install-miniboard-ipd.sh | sudo sh -s -- --show qr
```

如果页面在固定端口上，把 URL 模板写在 `--show` 后面：

```sh
curl -fsSL https://raw.githubusercontent.com/kxn/msu2-ip-display/master/scripts/install-miniboard-ipd.sh | sudo sh -s -- --show 'qr:http://{ip}:8080/'
```

`{ip}` 会在运行时替换成实际 IPv4。二维码模板会在服务启动前校验长度，能放进小屏后才会进入运行状态。

## 固定网口

默认选择带默认路由的 IPv4。现场有多张网卡，或者明确只想显示某个接口时，可以在安装时指定接口：

```sh
curl -fsSL https://raw.githubusercontent.com/kxn/msu2-ip-display/master/scripts/install-miniboard-ipd.sh | sudo sh -s -- --interface eth0
```

参数会写入系统服务。重新执行安装命令就是升级，也会更新服务参数。

## 屏幕状态

| 屏幕内容 | 含义 |
| --- | --- |
| 待机动画 | 小屏已刷写资源，host 程序还没有接管显示 |
| `获取IP中` | 已连接小屏，正在等待可显示的 IPv4 |
| `DHCP失败` | 当前只有链路本地地址，或者长时间没有拿到可用 DHCP 地址 |
| 两行数字 IP | 已获取 IPv4，直接照着输入即可 |
| 二维码 | 已获取 IPv4，扫码打开模板生成的 URL |

## 常见操作

查看版本：

```sh
miniboard-ipd --version
```

前台运行，方便临时测试：

```sh
miniboard-ipd run
miniboard-ipd run --show qr
miniboard-ipd run --interface eth0
```

排查问题时启用详细日志：

```sh
curl -fsSL https://raw.githubusercontent.com/kxn/msu2-ip-display/master/scripts/install-miniboard-ipd.sh | sudo sh -s -- --debug
```

OpenRC 系统会把服务输出写到 `/var/log/miniboard-ipd.log`。其他 init 系统可以用对应的服务管理命令查看状态：

```sh
miniboard-ipd status
```

卸载服务和已安装的 binary：

```sh
sudo miniboard-ipd uninstall
```

## 下载内容

[GitHub Release](https://github.com/kxn/msu2-ip-display/releases/latest) 会提供：

| 用途 | 文件 |
| --- | --- |
| Linux host 程序 x86_64 | `miniboard-ipd-linux-amd64.tar.gz` |
| Linux host 程序 ARM64 | `miniboard-ipd-linux-arm64.tar.gz` |
| Linux host 程序 ARMv7 32-bit | `miniboard-ipd-linux-arm32.tar.gz` |
| Windows 刷写器 | `MSU2.Flasher-windows-x64.exe` |
| Linux 刷写器 | `MSU2.Flasher-linux-x64` |
| macOS 刷写器 | `.dmg` |

## 开发者入口

开发、调试、协议资料和发布流程见 [DEVELOPER.md](DEVELOPER.md)。
