# 02 · 技术栈

- Tauri v2
- React 18 + TypeScript + Vite
- pnpm
- Tailwind CSS + shadcn/ui
- Zustand
- Zod
- `@tauri-apps/api` invoke/event
- `tauri-plugin-store`
- `tauri-plugin-log`
- `tauri-plugin-updater`
- Rust tokio、tokio-serial、tokio-modbus RTU、serialport、serde、thiserror、tracing
- 测试：Vitest/RTL、cargo test、cargo clippy

避免：

- 前端轮询 invoke 拉实时数据。
- TS 复制温度/PID 范围常量。
- Rust 跨 `.await` 持有阻塞锁访问串口。
- 配置/预设自写相对路径。
