# Host USB IP Display Draft

> Superseded by `docs/superpowers/specs/2026-07-17-host-usb-ip-display-design.md`.

本文是 Linux host-side IP 显示程序的需求和设计草案，供继续讨论。这里先固定目前已经确认的方向，不作为最终实现计划。

## 目标

在无头 Linux 设备上运行一个后台程序。用户把 MSU2 MINI USB 小屏插入设备后，程序自动连接屏幕，并在屏幕上显示这台设备当前可访问的 IPv4 地址，方便部署现场做网络配置和访问。

这个程序只解决“现场看 IP”这一件事。它不是桌面上位机，不提供通用屏幕控制 UI，也不做完整 Flash 恢复。

## 运行环境

- 仅面向 Linux。
- 同一时间默认只插一个目标 USB 屏幕。
- 同架构二进制应尽量跨发行版通用，避免要求用户在目标机器上重新编译。
- 依赖尽可能少，优先直接使用 Linux 系统接口。
- OpenWrt 这类小设备系统需要纳入服务安装覆盖范围；它们的 IP 选择策略可以通过 v1 的固定接口配置解决，未来再做 OpenWrt 专用策略。

## 程序形态

建议程序名暂定为 `miniboard-ipd`，提供这些命令：

```text
miniboard-ipd run
miniboard-ipd install
miniboard-ipd uninstall
miniboard-ipd status
```

`run` 前台运行，方便调试。`install` 根据当前系统 init 方式安装服务。

v1 不引入配置文件。所有可配置项都通过命令行参数指定；执行 `install` 时，安装器把当次命令行参数固化进对应 init 脚本或 service 文件。

固定接口配置 v1 就支持：

```text
miniboard-ipd run --interface eth0
miniboard-ipd install --interface eth0
```

如果设置了 `--interface`，IP 选择只看这个接口，不再自动猜测其它接口。

## 技术选型

推荐使用 Rust 写一个低依赖 daemon：

- 串口、USB 事件、IP 获取都优先直接用 Linux API。
- 参数解析手写或极小依赖。
- release 使用 `musl` 静态链接，按架构分发。
- 首批目标架构：`x86_64` 和 `aarch64`；如果目标设备需要，再补 `armv7` 32 位版本。
- 不使用 GUI、Tauri、Python runtime、libudev 强依赖或大型 async runtime。

备选方案：

- C：二进制最小，但状态机、错误处理和测试成本更高。
- Go：部署简单，但二进制通常更大。
- Rust + 大量 crate：开发更快，但不符合低依赖和小体积目标。

## 调研结论

设计前针对 Linux 服务安装、USB/TTY 断连和 IP 获取做了一轮资料核对：

- systemd：自定义 unit 放在 `/etc/systemd/system/`，service unit 以 `[Unit]`、`[Service]`、`[Install]` 为核心结构，适合把 `install` 时的参数直接固化到 `ExecStart`。
  - 参考：<https://docs.redhat.com/en/documentation/red_hat_enterprise_linux/9/html/using_systemd_unit_files_to_customize_and_optimize_your_system/assembly_working-with-systemd-unit-files_working-with-systemd>
- OpenRC：Alpine 文档确认服务运行控制使用 `rc-service`，开机启用使用 `rc-update`，服务脚本位于 init/runlevel 体系内。
  - 参考：<https://docs.alpinelinux.org/user-handbook/0.1a/Working/openrc.html>
- OpenWrt procd：OpenWrt 源码里的 `procd.sh` 提供 `procd_open_instance`、`procd_set_param command`、`respawn`、`netdev` 等服务管理接口，适合做小设备自动安装目标。
  - 参考：<https://git.openwrt.org/openwrt/openwrt/tree/?path=package%2Fsystem%2Fprocd%2Ffiles%2Fprocd.sh>
- rtnetlink：`RTM_GETADDR`/`RTM_GETROUTE` 可读取接口地址和路由信息，地址消息包含接口 index、family、scope、flags 等字段，适合做默认路由和 DHCP 来源判断的主路径。
  - 参考：<https://man7.org/linux/man-pages/man7/rtnetlink.7.html>
- `getifaddrs`：可以低依赖枚举本机接口名、接口 flag、地址和 netmask，可作为简单地址枚举路径或 fallback。
  - 参考：<https://man7.org/linux/man-pages/man3/getifaddrs.3.html>
- `poll`/TTY：`POLLERR`、`POLLHUP` 等事件可以提示 fd 错误或 hangup；Linux TTY 文档也说明 TTY/USB Serial 设备有 probe/remove/operation 这类生命周期。因此 USB 拔出设计不能只依赖 uevent，也要把 fd 错误和连续超时纳入断连判定。
  - 参考：<https://man7.org/linux/man-pages/man2/poll.2.html>
  - 参考：<https://docs.kernel.org/driver-api/tty/index.html>

基于这些资料，当前需求文档没有发现阻塞设计的问题。剩余不确定项都可以在实现阶段用单元测试、Linux VM 测试和实机拔插测试验证。

## USB 和串口

设备匹配：

- VID/PID: `1A86:FE0C`
- 串口参数：`921600 8N1 + RTS/CTS`
- 握手：发送并等待 `00 4D 53 4E 43 4E`，即 `\0MSNCN`

检测方式：

1. 优先用 Linux netlink uevent 监听 USB/tty add/remove。
2. 如果系统或权限不支持事件监听，退化为轮询 `/sys/class/tty`，频率控制在 `0.5s..1s`。
3. 找到候选 tty 后，向上查 sysfs USB 节点的 `idVendor`/`idProduct`。
4. 连接失败、握手失败或写入失败时关闭串口，回到监听状态。

## USB 拔出和错误处理

Linux 上 USB 串口被拔掉时，不同内核、驱动和时机可能表现不同。程序不能假设一定先收到某一种通知。

可能出现的信号：

- netlink uevent 收到目标 tty/USB 设备 `remove`。
- 串口 fd 上的 `poll`/`select` 返回 `POLLHUP` 或 `POLLERR`。
- `read`/`write` 返回错误，例如 `EIO`、`ENODEV`、`ENXIO` 或类似“设备不存在/IO 错误”。
- 写命令后不再收到协议回包，只表现为连续超时。
- `/dev/tty*` 节点消失，或 fd 仍在但后续读写失败。

设计上把这些都归一成一个内部事件：`DeviceDisconnected`。

处理流程：

1. 连接状态下，USB 事件监听和串口读写都可以触发断连判断。
2. 一旦判断断连，立即停止当前设备 session，关闭 fd，清空当前 IP/屏幕状态缓存。
3. 不尝试在拔掉的设备上显示“未连接”；设备已经不存在，写屏幕没有意义。
4. 回到监听状态，等待下一次插入。
5. 为避免接触不良导致日志刷屏，断连后的重新扫描使用短退避，例如 `200ms -> 500ms -> 1s`，上限 `1s`。
6. 如果只是单次协议超时，不立刻判定断连；连续若干次 keepalive/写入超时后再断开。建议阈值为 `3` 次。
7. 新设备插入后重新握手；即使系统复用了同一个 `/dev/ttyUSB0` 名字，也按全新设备处理。

日志策略：

- 正常拔插只记一条 info：`device disconnected` / `device connected`。
- 单次超时记 debug 或 suppress。
- 连续超时导致断连时记 warn，包含 tty 名和最近一次错误。

这样可以覆盖“先收到 remove”和“先写失败”两种情况，也能处理 USB 线松动、设备重启、驱动重新枚举等实际场景。

## 屏幕资源和页号

屏幕尺寸为 `160x80`，彩色图为 RGB565 big-endian。单张全屏图大小为 `25,600` bytes，占 `100` 个 Flash page。

当前紧凑布局页号：

| 页范围 | 内容 |
| --- | --- |
| `0..99` | 未连接/待机动图可见帧 |
| `100..199` | 未连接/待机动图空白帧 |
| `200..299` | 离线静图，资源目录 E1 指向这里 |
| `300..399` | 未获取 IP / 获取 IP 中状态图 |
| `400..499` | `DHCP失败` 状态图 |
| `500..599` | IP 显示空白背景 |
| `3820..3825` | 启动 logo，160x68 mono |
| `4026+` | 官方 `N24X33P` 数码管资源，必须保留 |

host-usb v1 在紧凑布局后的页号为：获取 IP 中 page `300`，DHCP 失败 page `400`，IP 背景 page `500`，数码管仍是 `4026 + digit`。旧布局里的 `3726`、`3826`、`3926` 只作为历史记录保留，活动代码不再使用。

## 屏幕状态

v1 使用预刷入的页面显示状态，不在运行时上传整张文字图。后续如果需要覆盖更多低频异常状态，可以再增加“从 USB 直接发送临时图像”的能力。

程序运行时有三个屏幕状态：

1. **未获取 IP**：目标网络接口没有可用 IPv4，显示 page `300`。
2. **DHCP 失败**：只拿到 `169.254.0.0/16` link-local IPv4，显示 page `400`。
3. **获得 IP**：拿到正常 IPv4，显示 page `500` 背景，并用官方 `N24X33P` 数码管显示 IP。

设备未插入时，程序不显示任何内容，只保持监听；屏幕自身会回到已刷入的未连接/待机动图。

内部状态可以比屏幕状态更细，例如“等待 DHCP 稳定”“多个地址但未确定 DHCP 来源”。这些内部状态在 v1 都继续显示 page `300`，只有确认失败后才切到 page `400`。

## IP 显示方案

获得 IP 后只显示 IP，不显示额外标签。

布局：

- IPv4 拆成两行：`octet0.octet1` 和 `octet2.octet3`。
- 每行独立按实际宽度水平居中。
- 官方数字 glyph：`24x33`。
- 点号没有官方资源，使用小像素点直接写 LCD 区域。
- 两行高度为 `33 + 8 + 33 = 74`，垂直起点 `y=3`。

示例：

```text
192.168
1.204
```

最宽情况：

```text
255.255
255.255
```

这已经在实机上看过，可读性良好。

## IPv4 选择策略

默认只显示 IPv4，不显示 IPv6。

建议策略：

1. 如果配置了固定接口，则只从该接口取 IPv4。
2. 否则优先取默认路由所在接口的正常 IPv4。
3. 如果没有默认路由，但系统只有一个正常 IPv4，则显示它，覆盖“不出网局域网但 DHCP 分配了地址”的场景。
4. 如果没有默认路由且存在多个正常 IPv4 候选，则优先显示 DHCP 来源的接口地址。
5. 如果没有默认路由、存在多个正常 IPv4，但没有任何 DHCP 来源线索，则在等待稳定期后按失败处理。
6. 如果只有 `169.254.0.0/16` 地址，则在等待稳定期后判定为 DHCP 失败，不显示具体地址。
7. 如果没有 IPv4，显示未获取 IP。

正常 IPv4 默认排除：

- `0.0.0.0`
- `127.0.0.0/8`
- `169.254.0.0/16`
- multicast 和其它明显不可用于现场访问的地址

自动 fallback 时还应排除明显虚拟接口，例如：

```text
lo, docker*, br-*, veth*, virbr*, tun*, tap*, wg*
```

DHCP 来源线索优先从 rtnetlink 地址信息判断，例如地址 lifetime/flag 是否表现为动态地址；如果不同发行版表现不一致，再补充读取常见 DHCP lease 文件作为 fallback。v1 不依赖 NetworkManager、systemd-networkd 或 dhcpcd 的专用 DBus/API。

## DHCP 失败判断

Linux 内核本身不会统一像 Windows 那样自动分配 APIPA，但 NetworkManager、systemd-networkd、avahi-autoipd 等用户态组件可能配置 `169.254.x.x`。

默认策略是不显示 `169.254.x.x`，而显示 `DHCP失败`。理由是现场用户看到一个 IP 往往会认为网络已经可用，而 link-local 地址通常表示 DHCP 没拿到期望的网络配置。

为了避免“刚插上先显示 DHCP 失败，过几秒又显示 IP”的跳变，v1 需要有保守的失败延迟：

- 设备刚连接、网络刚变化、接口刚 up 时，先显示 `未获取 IP`。
- 只有 link-local 地址或多个正常 IPv4 但没有 DHCP 来源时，先进入内部 pending 状态。
- pending 状态稳定超过配置时间后才切到 `DHCP失败`。默认使用 `45s`。
- 一旦拿到可显示 IPv4，立即切到 IP 显示。

这不能从理论上完全阻止极慢 DHCP 在失败页之后才成功，但可以避免常见启动过程中的误判。若现场网络经常慢，可以把失败延迟调大。

可选命令行参数：

```text
--dhcp-fail-delay-seconds 45
```

未来如果需要直连维护模式，可以增加配置：

```text
allow_link_local=true
```

但 v1 不启用。

## Keepalive

固件在一段时间没有收到命令后可能回到动图状态。host 程序连接设备后需要定期发送轻量 keepalive。

建议：

- IP 已显示时，只更新一个不影响观感的小像素区域。
- 状态图显示时，可以重发当前 page 或同样更新小区域。
- 周期先按 `800ms` 实现，和 flasher 的已验证 keepalive 行为保持一致；后续根据实机观察调整。

## 服务安装

`install` 根据系统 init 自动选择：

v1 自动安装优先覆盖这些 init：

| init | 常见系统 | v1 行为 |
| --- | --- | --- |
| systemd | Debian/Ubuntu、RHEL/Fedora、Arch、现代 Yocto 等 | 写入 `/etc/systemd/system/miniboard-ipd.service`，执行 `systemctl enable --now` |
| OpenRC | Alpine、Gentoo、部分 Artix/嵌入式系统 | 写入 `/etc/init.d/miniboard-ipd`，执行 `rc-update add` 和 `rc-service start` |
| OpenWrt procd | OpenWrt/ImmortalWrt | 写入 `/etc/init.d/miniboard-ipd` procd 脚本，执行 `enable` 和 `start` |
| SysV/initscripts | 老 Debian/RHEL、部分 Yocto/Buildroot 派生系统 | 写入 `/etc/init.d/miniboard-ipd`，优先 `update-rc.d`，其次 `chkconfig` |
| BusyBox init | Buildroot、部分极简嵌入式 Linux | best-effort 写入 `S99miniboard-ipd` 到已有 rc 目录，例如 `/etc/init.d/` 或 `/etc/rc.d/` |

v1 可识别但不自动完整安装的 init：

| init | 处理 |
| --- | --- |
| runit | 输出 `/etc/sv/miniboard-ipd/run` 模板和启用提示 |
| s6 / s6-rc | 输出 service 模板和手动安装提示 |
| dinit | 输出 service 模板和手动安装提示 |
| Upstart | 只提示不支持自动安装；属于遗留系统 |
| supervisord/cron | 不作为 init 支持，只可作为用户自定义启动方式 |

检测顺序建议：

1. OpenWrt/procd：检测 `/sbin/procd`、`/etc/openwrt_release` 或 `/etc/init.d` 脚本风格。
2. systemd：检测 `/run/systemd/system` 和 `systemctl`。
3. OpenRC：检测 `/run/openrc/softlevel`、`rc-service` 或 `rc-update`。
4. SysV/initscripts：检测 `update-rc.d`、`chkconfig`、`service`。
5. BusyBox init：检测 `/proc/1/comm`、`/bin/busybox`、常见 rc 目录。
6. 其它 init：识别后输出模板；无法识别时只安装 binary 并打印手动启动说明。

安装路径由实现按 Linux 惯例选择，默认建议 `/usr/local/bin/miniboard-ipd`。`install`/`uninstall` 需要 root。普通 `run` 可以前台执行，用于调试。`install` 不读取配置文件；它把参数直接写入启动命令，例如 systemd 的 `ExecStart=/usr/local/bin/miniboard-ipd run --interface eth0 --dhcp-fail-delay-seconds 45`。

## 实现注意事项

1. DHCP 来源判断优先使用 rtnetlink 动态地址信息；如果实测覆盖不够，再补充解析常见 lease 文件。
2. BusyBox init 自动安装只能 best-effort，不同极简系统 rc 目录差异较大。
3. runit/s6/dinit v1 只输出模板，不做完整自动安装。
4. keepalive 周期先按 `800ms` 实现，后续根据实机长期观察调整。
5. release 产物优先 `x86_64-unknown-linux-musl` 和 `aarch64-unknown-linux-musl`，必要时补 `armv7-unknown-linux-musleabihf` 或实际可用的 armv7 musl target。
6. 默认安装路径建议 `/usr/local/bin/miniboard-ipd`，除非目标系统没有该目录。
