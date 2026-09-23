# 08 · 发布与 CI/CD

## CI（`.github/workflows/ci.yml`）

PR 与 main 推送触发，三个并行作业：

| 作业             | 环境    | 内容                                                                                       |
| ---------------- | ------- | ------------------------------------------------------------------------------------------ |
| Frontend         | ubuntu  | `pnpm lint`、`format:check`、`test:coverage`（覆盖率门槛）、`build`                        |
| Rust             | windows | `cargo fmt --check`、`clippy --all-targets --locked -D warnings`、`test --locked`          |
| Dependency audit | ubuntu  | `pnpm audit --audit-level moderate`、`cargo audit`（豁免见 `src-tauri/.cargo/audit.toml`） |

`ci.yml` 同时是可复用工作流（`workflow_call`），发版时对发版提交完整重跑。

## 发版（`.github/workflows/release.yml`，release-please）

1. main 上每次合并后，release-please 根据 Conventional Commits 维护一个 **Release PR**：计算版本、更新
   `CHANGELOG.md` 与版本号（`package.json`、`src-tauri/Cargo.toml`、`src-tauri/Cargo.lock`、`src-tauri/tauri.conf.json`）。
   配置见 `release-please-config.json`、`.release-please-manifest.json`。
2. **合并 Release PR 才会发版**：release-please 创建 **draft** Release 与 `vX.Y.Z` tag。
3. `Gate`：对发版提交复用 `ci.yml` 完整门禁。
4. `Build`：Windows（NSIS）与 macOS（universal）由 tauri-action 按 `releaseId` 上传到该 draft；资产名为
   `yudian-ai-tool_[version]_[platform]_[arch][ext]`（产品名为中文，直接使用会被 GitHub 改写为 `AI._...`）。
5. `Verify artifacts and publish`：核对资产齐全（`.exe`、`.dmg`、`.app.tar.gz` 及各自 `.sig`、`latest.json`）、
   `latest.json` 版本与发版版本一致、每个平台 URL 都指向真实资产，全部通过后才发布 draft 并标记为 Latest。

版本规则：`feat` → minor，`fix`/`perf`/`refactor` → patch；1.0 之前破坏性变更只升 minor。需要指定版本时在提交正文加
`Release-As: x.y.z`。

注意：Release PR 由 `GITHUB_TOKEN` 创建，不会触发 PR 上的 CI；发版提交在第 3 步完整重跑门禁。

## Secrets

- `TAURI_SIGNING_PRIVATE_KEY`、`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`：updater 签名（公钥在 `tauri.conf.json`）。
- `APPLE_CERTIFICATE`、`APPLE_CERTIFICATE_PASSWORD`、`APPLE_SIGNING_IDENTITY`、`APPLE_ID`、`APPLE_PASSWORD`、
  `APPLE_TEAM_ID`：配置后 macOS 走签名与公证构建，未配置时产出未签名包。

## 依赖更新

Dependabot 每周为 npm、cargo、GitHub Actions 开分组 PR（minor + patch）。
