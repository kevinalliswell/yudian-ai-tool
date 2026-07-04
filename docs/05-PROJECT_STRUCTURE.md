# 05 · 项目结构

- `src/app`：应用布局和 Tab 容器。
- `src/features`：业务模块预留。
- `src/components/ui`：shadcn/ui 拷贝组件。
- `src/stores`：Zustand store。
- `src/lib/api.ts`：唯一 invoke/listen 出入口。
- `src/mocks`：前端 mockApi 与 snapshots。
- `src-tauri/src/modbus`：寄存器、换算、校验纯逻辑。
- `src-tauri/src/backend`：DeviceBackend、Mock、Real。
- `src-tauri/src/device`：DeviceActor。
- `src-tauri/src/commands.rs`：薄 Tauri 命令层。

Rust/TS 边界使用 camelCase serde 对齐。
