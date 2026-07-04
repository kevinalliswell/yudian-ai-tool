# 宇电 AI 仪表桌面工具 · 文档总纲

本项目从零重写宇电 AI-516 / AI-516P / AI-518 / AI-518P 系列温控仪表上位机工具。
唯一继承物是功能范围和 Modbus RTU 通讯协议。

核心原则：

- 协议换算、校验、寄存器映射必须与 `04-MODBUS_PROTOCOL.md` 对齐。
- 所有总线访问必须经 Rust `DeviceActor` 串行化。
- 实时数据由 Rust `emit` 到前端 `listen`，前端不轮询 invoke。
- 配置与曲线预设走 `tauri-plugin-store`。
- 校验范围由 Rust `ValidationLimits` 单一来源提供。
- 无硬件也能开发：前端 `mockApi` 与 Rust `MockBackend` 两层 mock。
- 不保留 Python 文件。

阅读顺序：00 总纲、01 产品、02 技术栈、03 架构、04 协议、05 结构、06 Mock、07 测试、08 发布、09 坑点。
