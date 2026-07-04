# 06 · 开发与 Mock

## Rust MockBackend

`YUDIAN_BACKEND=mock pnpm tauri dev`：完整 Tauri 命令流和事件流，但不碰真实串口。

`YUDIAN_BACKEND=mock-offline`：离线/超时场景。

## 前端 mockApi

`VITE_MOCK=true pnpm dev`：纯浏览器运行，不需要 Tauri 或硬件。mockApi 用事件模拟
`device://reading`，这是 mock 环境特例，生产实时数据来自 Rust。

Snapshots 位于 `src/mocks/snapshots/`。
