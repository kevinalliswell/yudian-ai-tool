# 宇电 AI 温控工具

宇电 AI-516 / AI-516P / AI-518 / AI-518P 系列温控仪表上位机工具。技术栈为 Tauri v2、
React/TypeScript/Vite 和 Rust。

## 功能

- 串口扫描、连接/断开、型号识别。
- PV/SV/MV 实时监控，数据由 Rust 后台任务通过 `device://reading` 事件推送。
- SP1、PID 和运行/保持/停止控制。
- 温控曲线编辑、预设保存、上传和下载。
- Rust 单一 `DeviceActor` 串行化所有 Modbus 总线访问。
- 纯前端 mock 与 Tauri mock 两种无硬件开发路径。

## 开发

PowerShell 若拦截 `pnpm.ps1`，使用 `pnpm.cmd ...` 或 `corepack.cmd pnpm ...`。

```powershell
pnpm install
pnpm dev
$env:VITE_MOCK="true"; pnpm dev
$env:YUDIAN_BACKEND="mock"; pnpm tauri dev
pnpm tauri dev
```

## 测试

```powershell
pnpm lint
pnpm test
pnpm build
cd src-tauri
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

## 打包

```powershell
pnpm tauri icon src-tauri/icons/icon.png
pnpm tauri build
```

Windows 会在 `src-tauri/target/release/bundle/` 下生成安装包。macOS `.app` / `.dmg` 需在 macOS
机器或 GitHub Actions 的 `macos-latest` runner 上构建。

macOS 未签名包首次运行可能被 Gatekeeper 拦截，可使用右键打开，或在终端运行：

```bash
xattr -cr "宇电 AI 温控工具.app"
```

## 发布与签名

Release workflow 需要以下 GitHub Secrets：

- `TAURI_SIGNING_PRIVATE_KEY`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
- `APPLE_CERTIFICATE`
- `APPLE_CERTIFICATE_PASSWORD`
- `APPLE_SIGNING_IDENTITY`
- `APPLE_ID`
- `APPLE_PASSWORD`
- `APPLE_TEAM_ID`

`src-tauri/tauri.conf.json` 里只存 updater 公钥；私钥不要入库。

发版由 release-please 驱动：合并 main 后自动维护 Release PR，合并该 PR 才会构建、校验产物并发布。
完整流程见 [docs/08-RELEASE_CICD.md](docs/08-RELEASE_CICD.md)。

如果未配置 Apple 证书相关 secrets，Release workflow 会产出未签名 macOS 包；配置后自动走签名构建。
