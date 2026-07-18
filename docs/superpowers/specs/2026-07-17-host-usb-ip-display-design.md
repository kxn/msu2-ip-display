# Host USB IP 显示程序设计

## 状态

本设计已从讨论草案收敛，进入实现计划前的用户 review 阶段。

本文覆盖 `host-usb/` 下的 Linux host-side 程序，以及已经加入 flasher 的配套 MSU2 MINI 屏幕素材。

## 目标

在无头 Linux 设备上运行一个小型后台程序。用户把 MSU2 MINI USB 小屏插入设备后，程序自动连接屏幕，并显示这台设备当前可用于现场访问的 IPv4 地址。

主要场景是 DHCP 部署：设备到现场后没有显示器，用户需要快速知道它拿到的 IP，以便继续配置或远程登录。

## 非目标

- 不做桌面 GUI。
- 不做通用 MSU2 控制面板。
- 不做完整 Flash 备份或出厂恢复工具。
- v1 不显示 IPv6。
- v1 不在运行时上传任意文字状态图。
- v1 不引入配置文件。

## 已确认硬件事实

实现必须复用当前 flasher 已验证过的协议事实：

- USB VID/PID: `1A86:FE0C`
- 串口参数：`921600 8N1 + RTS/CTS`
- 握手：`00 4D 53 4E 43 4E`，即 `\0MSNCN`
- 屏幕：`160x80`
- 全屏彩色图：RGB565 big-endian，`25,600` bytes，占 `100` 个 Flash page
- IP 空白背景页：`3926`
- 官方 `N24X33P` 数码管资源起始页：`4026`

已经在实机上验证过的 IP 版式是两行居中显示，最宽情况为：

```text
255.255
255.255
```

这个版式可读性良好。

## 配套 flasher 素材

flasher 现已加入一张新的状态图：

- 标签：`dhcp_failed`
- 页范围：`3726..3825`
- 刷写素材：`flasher/src-tauri/assets/dhcp_failed.rgb565be`
- 预览图：`docs/mockups/msu2-dhcp-failed.png`
- 文案：`DHCP失败`
- 风格：沿用现有 `未连接`、`获取IP中` 的黑底绿色状态图样式

当前固定页布局：

| 页范围 | 内容 |
| --- | --- |
| `0..3599` | 未连接/待机动图，36 帧 |
| `3726..3825` | DHCP 失败状态图 |
| `3826..3925` | 未获取 IP / 获取 IP 中状态图 |
| `3926..4025` | IP 显示空白背景 |
| `4026+` | 保留官方数字/字库资源 |

`3726..3825` 使用当前固定布局里的空档。官方示例曾在附近页放过其它 demo 资源，因此这个选择需要持续记录。host-side IP 显示依赖的是 `4026+` 的 24x33 数字资源，不依赖旧的大号数码管示例资源。

## 程序形态

程序名暂定为 `miniboard-ipd`。

命令：

```text
miniboard-ipd run [options]
miniboard-ipd install [options]
miniboard-ipd uninstall
miniboard-ipd status
```

`run` 前台运行，是主要调试入口。

`install` 安装当前 binary，并写入 service/init 脚本。它不读取也不写入配置文件；执行 `install` 时传入的参数会被固化到安装后的启动命令里。

示例：

```text
miniboard-ipd run --interface eth0
miniboard-ipd install --interface eth0 --dhcp-fail-delay-seconds 45
```

v1 支持的命令行参数：

| 参数 | 默认值 | 含义 |
| --- | ---: | --- |
| `--interface <name>` | 未设置 | 只显示这个接口上的 IPv4 |
| `--dhcp-fail-delay-seconds <n>` | `45` | pending 状态稳定多久后显示 DHCP 失败 |

默认安装路径建议为 `/usr/local/bin/miniboard-ipd`。如果目标系统没有这个目录，实现可以选择符合该系统习惯的位置。

## 技术选型

推荐使用低依赖 Rust daemon。

理由：

- 当前 flasher 的协议实现已经是 Rust。
- Rust 比 C 更适合写长期运行的状态机、错误处理和测试。
- 使用 `musl` 静态链接后，同架构二进制可尽量跨发行版使用。
- 避免大型运行时依赖，有利于无头设备和小设备部署。

实现约束：

- 不使用 Tauri。
- 不依赖 Python runtime。
- 不强依赖 `libudev`。
- 除非简单事件循环不够用，否则不引入大型 async runtime。
- 优先直接使用 Linux API：termios、sysfs、netlink、poll。

release 目标：

- 必做：`x86_64-unknown-linux-musl`
- 必做：`aarch64-unknown-linux-musl`
- 按需补充：ARMv7 32 位 musl 目标，例如 `armv7-unknown-linux-musleabihf`，最终以实际目标硬件和工具链可用性为准

## 调研依据

实现前已核对这些一手或官方资料：

- systemd：自定义 unit 放在 `/etc/systemd/system/`，service unit 由 `[Unit]`、`[Service]`、`[Install]` 等 section 组成，适合把 install 时的参数固化到 `ExecStart`。
  - <https://docs.redhat.com/en/documentation/red_hat_enterprise_linux/9/html/using_systemd_unit_files_to_customize_and_optimize_your_system/assembly_working-with-systemd-unit-files_working-with-systemd>
- OpenRC：Alpine 文档确认运行服务使用 `rc-service`，开机启用使用 `rc-update`。
  - <https://docs.alpinelinux.org/user-handbook/0.1a/Working/openrc.html>
- OpenWrt procd：OpenWrt 源码中的 `procd.sh` 提供 `procd_open_instance`、`procd_set_param command`、`respawn`、`netdev` 等能力，适合小设备服务安装。
  - <https://git.openwrt.org/openwrt/openwrt/tree/?path=package%2Fsystem%2Fprocd%2Ffiles%2Fprocd.sh>
- rtnetlink：`RTM_GETADDR`/`RTM_GETROUTE` 可读取接口地址和路由信息，地址消息包含接口 index、family、scope、flags 等字段，适合做默认路由和动态地址判断。
  - <https://man7.org/linux/man-pages/man7/rtnetlink.7.html>
- `getifaddrs`：可低依赖枚举接口名、接口 flag、地址和 netmask，可作为简单地址枚举路径或 fallback。
  - <https://man7.org/linux/man-pages/man3/getifaddrs.3.html>
- `poll`/TTY：`POLLERR`、`POLLHUP` 等事件可提示 fd 错误或 hangup；Linux TTY 文档说明 TTY/USB Serial 设备有 probe/remove/operation 生命周期。因此 USB 拔出不能只依赖 uevent。
  - <https://man7.org/linux/man-pages/man2/poll.2.html>
  - <https://docs.kernel.org/driver-api/tty/index.html>

这些资料没有暴露需求级阻塞点。剩余不确定项可以在实现阶段通过单元测试、Linux VM 测试和实机拔插测试验证。

## 模块结构

建议目录结构：

```text
host-usb/
  Cargo.toml
  src/
    main.rs
    cli.rs
    daemon.rs
    device_scan.rs
    usb_events.rs
    serial.rs
    protocol.rs
    display.rs
    ip_detect.rs
    service_install.rs
    logging.rs
```

职责划分：

- `cli`：手写命令行参数解析和校验。
- `daemon`：顶层状态机和事件循环。
- `device_scan`：扫描 sysfs tty 设备并匹配 VID/PID。
- `usb_events`：监听 netlink uevent；不可用时允许退化到轮询。
- `serial`：termios 配置、读写超时、断连错误分类。
- `protocol`：MSU2 握手、显示 page、设置 XY/Size、RAM 初始化/叠加/显示、直接像素写入。
- `display`：高层屏幕状态和 IP 数码管布局。
- `ip_detect`：IPv4、默认路由、DHCP/动态来源选择。
- `service_install`：init 检测、install、uninstall、status。
- `logging`：简单 stdout/stderr/syslog 友好的日志。

## 运行状态机

顶层状态：

```text
Listening
  -> Connecting
  -> ConnectedPendingIp
  -> ConnectedDhcpFailed
  -> ConnectedShowingIp
  -> Listening
```

流程：

1. 启动后进入 `Listening`。
2. 优先监听 USB uevent；不可用时每 `500..1000ms` 轮询 sysfs。
3. 找到匹配 tty 后，用 `921600 8N1 + RTS/CTS` 打开。
4. 发送 `\0MSNCN` 握手。
5. 握手成功后显示 page `3826`，进入 `ConnectedPendingIp`。
6. 周期性评估 IPv4 选择结果。
7. 如果出现可显示 IPv4，显示 page `3926`，绘制两行 IP，进入 `ConnectedShowingIp`。
8. 如果失败证据稳定超过延迟，显示 page `3726`，进入 `ConnectedDhcpFailed`。
9. 如果设备断开，关闭 fd、清空缓存，回到 `Listening`。

USB 屏已拔掉时，不尝试显示 `未连接`。设备已经不存在，写屏幕没有意义；屏幕固件会回到已刷入的待机/未连接帧。

## USB 检测

优先路径：

1. 监听 `NETLINK_KOBJECT_UEVENT`。
2. 收到 tty/USB add/remove 事件后重新扫描目标设备。
3. 从 tty 的 sysfs 路径向上查找 USB 设备属性。
4. 匹配 `idVendor=1a86`、`idProduct=fe0c`。

fallback 路径：

1. 每 `500..1000ms` 轮询 `/sys/class/tty`。
2. 使用同样的 sysfs parent 查找和 VID/PID 匹配逻辑。

v1 假设同时只插一个目标屏幕。如果发现多个候选，选择第一个稳定候选并记录 warning。

## USB 拔出处理

USB 移除在 Linux 上可能通过多种方式表现。以下情况都归一成内部事件 `DeviceDisconnected`：

- 当前 tty 或 USB parent 收到 uevent `remove`。
- `poll`/`select` 返回 `POLLHUP`、`POLLERR` 或 `POLLNVAL`。
- `read`/`write` 返回 `EIO`、`ENODEV`、`ENXIO` 或类似设备消失错误。
- 协议写入或 keepalive 连续超时。
- `/dev/tty*` 节点消失或被重新枚举替换。

规则：

- 单次超时不立刻判定断连。
- 连续 3 次 keepalive/协议超时后按断连处理。
- 判定断连后立即关闭 fd，丢弃当前设备状态。
- 断连后重新扫描使用短退避：`200ms -> 500ms -> 1s`，上限 `1s`。
- 设备重新插入后必须重新握手，即使 tty 名字仍是 `/dev/ttyUSB0`。

日志：

- 正常连接/断开：记一条 info。
- 单次超时：debug 或不输出。
- 连续超时导致断连：warning，包含 tty 和最近错误。

## 屏幕状态

v1 屏幕状态：

| 内部状态 | 屏幕输出 |
| --- | --- |
| 尚无可显示 IPv4 | 显示 page `3826` |
| DHCP 失败已确认 | 显示 page `3726` |
| IPv4 可用 | 显示 page `3926`，再绘制 IP |

内部状态可以比屏幕状态更细。例如“等待 DHCP 稳定”和“多个地址但未确定 DHCP 来源”都继续显示 page `3826`。

## IP 绘制

IPv4 绘制规则：

- IPv4 拆成两行：`octet0.octet1` 和 `octet2.octet3`。
- 每行按实际字形宽度独立水平居中。
- 数字使用官方 `N24X33P`，页号为 `4026 + digit`。
- 数字尺寸为 `24x33`。
- 官方资源没有点号；点号用小块 RGB565 直接写 LCD 区域。
- 垂直布局：两行 `33px`，中间 `8px` 间距，起始 `y=3`。

除 keepalive 外，只有 IP 或屏幕状态变化时才重绘。

## IPv4 选择策略

v1 只显示 IPv4。

选择顺序：

1. 如果设置了 `--interface`，只考虑该接口。
2. 否则优先显示默认路由接口上的正常 IPv4。
3. 如果没有默认路由，但只有一个正常 IPv4 候选，则显示它。这个规则覆盖“不出网局域网但 DHCP 分配了地址”的场景。
4. 如果没有默认路由且存在多个正常 IPv4 候选，优先显示 DHCP/动态来源候选。
5. 如果多个正常候选都没有 DHCP/动态来源线索，则继续 pending；失败延迟到期后显示 DHCP 失败。
6. 如果只有 `169.254.0.0/16` 地址，则继续 pending；失败延迟到期后显示 DHCP 失败。
7. 如果没有 IPv4，继续 pending。

正常 IPv4 默认排除：

- `0.0.0.0`
- `127.0.0.0/8`
- `169.254.0.0/16`
- multicast 和其它明显不适合现场访问的地址

自动 fallback 时排除明显虚拟接口名：

```text
lo, docker*, br-*, veth*, virbr*, tun*, tap*, wg*
```

DHCP/动态来源判断：

- 主路径：rtnetlink 地址 flags/cache info。
- 如果实测覆盖不够，再补充解析常见 DHCP lease 文件。
- v1 不依赖 NetworkManager、systemd-networkd、dhcpcd 或 D-Bus API。

## DHCP 失败延迟

默认延迟：`45s`。

原因：

- 避免设备刚启动、网线刚插上、交换机协商较慢时立刻显示失败。
- 比 60 秒反馈更快。
- 可以通过 install 时的命令行参数覆盖。

规则：

- 当相关接口/网络状态进入失败倾向 pending 时开始计时。
- 一旦出现正常可显示 IPv4，立刻取消 pending 并显示 IP。
- USB 断开会清空延迟状态。
- 接口或 link 状态变化会重置 pending timer。

## Keepalive

MSU2 固件一段时间没有收到命令后可能回到离线动画。

v1 keepalive：

- 周期先按 `800ms`，和 flasher 的已验证 keepalive 行为保持一致。
- IP 显示状态下，更新一个不可见或不影响观感的小像素区域。
- 页面状态下，可重发当前 page，也可使用同样的小像素更新；最终以实机验证为准。
- keepalive 失败计入断连超时计数。

像素位置必须不影响 IP 或状态文字可读性。

## 服务安装

`install` 自动检测 init 并写入启动文件。命令行参数直接嵌入启动命令。

v1 必须自动支持：

| Init | 常见系统 | 安装行为 |
| --- | --- | --- |
| systemd | Debian/Ubuntu、RHEL/Fedora、Arch、Raspberry Pi OS、很多 Yocto 镜像 | 写 `/etc/systemd/system/miniboard-ipd.service`；执行 `systemctl daemon-reload`；执行 `systemctl enable --now miniboard-ipd.service` |
| OpenRC | Alpine、Gentoo、部分嵌入式系统 | 写 `/etc/init.d/miniboard-ipd`；执行 `rc-update add miniboard-ipd default`；执行 `rc-service miniboard-ipd start` |
| OpenWrt procd | OpenWrt/ImmortalWrt | 写 `/etc/init.d/miniboard-ipd`，使用 `/etc/rc.common` 和 `USE_PROCD=1`；执行 `enable` 和 `start` |
| SysV/initscripts | 老 Debian/RHEL、部分 Yocto/Buildroot 派生系统 | 写 `/etc/init.d/miniboard-ipd`；优先使用 `update-rc.d`，其次 `chkconfig` |
| BusyBox init | Buildroot 和极简嵌入式 Linux | best-effort 写 `S99miniboard-ipd` 到已有 rc 目录 |

v1 识别但不完整自动安装：

| Init | 行为 |
| --- | --- |
| runit | 输出 `/etc/sv/miniboard-ipd/run` 模板和启用提示 |
| s6 / s6-rc | 输出 service 模板和手动安装提示 |
| dinit | 输出 service 模板和手动安装提示 |
| Upstart | 输出遗留系统不支持提示 |
| supervisord/cron | 当作用户自定义启动方式，不视为系统 init |

检测顺序：

1. OpenWrt/procd
2. systemd
3. OpenRC
4. SysV/initscripts
5. BusyBox init
6. template-only init
7. unknown

`uninstall` 删除由 `install` 创建的文件，并在支持的 init 上 disable/stop 服务。

## 错误处理

daemon 应长期运行并能自恢复：

- 没有 USB 屏：保持 `Listening`。
- 串口打开失败：记录一次日志，继续监听。
- 握手失败：关闭候选设备，恢复扫描。
- 协议写入失败：分类为 transient 或 disconnect，不崩溃。
- USB 断开：关闭 fd 并重新监听。
- IP 检测失败：记录 warning，保持当前或 pending 屏幕状态。
- install 遇到不支持的 init：如果可以安装 binary，则输出手动说明；只有请求的安装动作无法完成时才返回非零。

## 测试计划

单元测试：

- CLI 参数解析和 install 命令固化。
- IP 选择顺序：无默认路由、只有 link-local、多个候选、固定接口。
- DHCP 失败延迟状态转换。
- IP 字形布局：短地址和最宽地址都居中。
- 协议包生成：show page、RAM digit add、RAM mix show、dot write。
- 断连分类：uevent、poll flag、I/O error、连续超时。
- init 检测和生成脚本内容。

集成测试：

- Linux VM 中测试 systemd 脚本生成。
- 可行时用容器或 VM 测试 OpenRC/SysV 脚本生成。
- 用 fixture 模拟 sysfs，测试 VID/PID 匹配。

硬件测试：

- 连接目标屏幕并验证握手。
- 显示 pending page、DHCP failed page、实际 IP。
- 连接状态下拔 USB，确认进程不崩溃并回到监听。
- 重新插入 USB，确认重新握手并重绘。
- 长时间显示 IP，确认 keepalive 阻止固件回到离线动画。

## 延后到实现阶段验证的问题

- rtnetlink flags 是否足够可靠地区分 DHCP/动态地址。
- 不同 BusyBox init 系统上的实际 rc 目录。
- keepalive 的具体像素位置和命令序列。
- ARMv7 release 最终使用哪个 musl target。
