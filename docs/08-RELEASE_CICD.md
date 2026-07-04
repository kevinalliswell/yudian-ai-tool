# 08 · 发布与 CI/CD

阶段：

1. 本地未签名包：`pnpm tauri icon ...`、`pnpm tauri build`。
2. 代码签名：Windows Authenticode，macOS Developer ID + notarization。
3. 自动更新：`tauri-plugin-updater`、Release `latest.json`、签名校验。

GitHub Actions：

- `ci.yml`：lint/test/build 门禁。
- `release.yml`：Conventional Commits 计算版本，tauri-action 构建 Windows/macOS，上传 Release 资产。

Secrets：`TAURI_SIGNING_PRIVATE_KEY`、Apple 证书与 notarization 相关变量。
