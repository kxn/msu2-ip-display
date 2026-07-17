# Host USB

这个目录预留给未来 host-side USB/串口通信程序。当前没有实现代码。

后续实现前先看：

- `docs/msu2-protocol-and-flash-layout.md`
- `references/vendor/source/msu2-lite-pro/`
- `references/vendor/msu2-programmable-usb-screen/MSU2 演示Demo及固件V1.0/Python源码/MSU2_DemoV1.0.py`

## 设计注意点

- 不要把 `19200` 当成全系列唯一串口速度；1.47/2.8 官方代码使用 `921600` 和 RTS/CTS。
- 协议层建议拆成独立包：握手、Flash、LCD、SFR/传感器、错误处理。
- 图像转换和串口发送应分层，避免把 UI、文件格式和协议包拼装混在一起。
- 写 Flash 前必须明确擦除范围，避免覆盖 `4026+` 字库/资源区。
