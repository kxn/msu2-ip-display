# References

这里放去重后的参考资料。大体积文件会纳入 git，因为它们是稳定输入，不应频繁变化。

## 目录

- `vendor/`: 官方 PDF、固件、Demo、工具和官方参考源码。
- `reverse-engineering/`: 之前逆向和实现过程中沉淀的文本资料。
- `artifacts/`: 生成资产、UI mock、验证日志等可复用产物。

## 约定

- 不放 `node_modules/`、`target/`、虚拟环境、临时反编译目录。
- 精确重复文件按 SHA-256 去重。
- 同名但 SHA-256 不同的固件保留。
