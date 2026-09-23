# Changelog

自 v0.6.0 起由 [release-please](https://github.com/googleapis/release-please) 根据 Conventional Commits 自动维护；更早的版本由提交历史整理。

## [0.5.0](https://github.com/kevinalliswell/yudian-ai-tool/releases/tag/v0.5.0) (2026-07-11)

### 新功能

- audit industrial control actions
- add structured audit log storage

## [0.4.9](https://github.com/kevinalliswell/yudian-ai-tool/releases/tag/v0.4.9) (2026-07-11)

### 重构

- split app shell feature panels

## [0.4.8](https://github.com/kevinalliswell/yudian-ai-tool/releases/tag/v0.4.8) (2026-07-11)

### 问题修复

- enable tauri content security policy

## [0.4.7](https://github.com/kevinalliswell/yudian-ai-tool/releases/tag/v0.4.7) (2026-07-11)

### 问题修复

- default missing curve time limits
- enforce the encoded curve time limit

## [0.4.6](https://github.com/kevinalliswell/yudian-ai-tool/releases/tag/v0.4.6) (2026-07-11)

### 问题修复

- reject non-finite control values

## [0.4.5](https://github.com/kevinalliswell/yudian-ai-tool/releases/tag/v0.4.5) (2026-07-11)

### 问题修复

- preserve long monitoring intervals during backoff
- back off and stop after monitor failures

## [0.4.4](https://github.com/kevinalliswell/yudian-ai-tool/releases/tag/v0.4.4) (2026-07-11)

### 问题修复

- keep unknown models read-only

### 重构

- use device write permission as source of truth

## [0.4.3](https://github.com/kevinalliswell/yudian-ai-tool/releases/tag/v0.4.3) (2026-07-11)

### 问题修复

- explain read-only write rejection
- keep devices read-only when DPT is unavailable

## [0.4.2](https://github.com/kevinalliswell/yudian-ai-tool/releases/tag/v0.4.2) (2026-07-11)

### 问题修复

- reset device backend after request timeout
- time out queued device requests

## [0.4.1](https://github.com/kevinalliswell/yudian-ai-tool/releases/tag/v0.4.1) (2026-07-11)

### 问题修复

- avoid float mismatch during curve verification
- guard run command with safety checks

## [0.4.0](https://github.com/kevinalliswell/yudian-ai-tool/releases/tag/v0.4.0) (2026-07-11)

### 新功能

- synchronize device parameters after connect

## [0.3.12](https://github.com/kevinalliswell/yudian-ai-tool/releases/tag/v0.3.12) (2026-07-11)

### 问题修复

- validate persisted curve presets

## [0.3.11](https://github.com/kevinalliswell/yudian-ai-tool/releases/tag/v0.3.11) (2026-07-11)

### 问题修复

- reject invalid curve data

## [0.3.10](https://github.com/kevinalliswell/yudian-ai-tool/releases/tag/v0.3.10) (2026-07-11)

### 问题修复

- make PID writes transactional

## [0.3.9](https://github.com/kevinalliswell/yudian-ai-tool/releases/tag/v0.3.9) (2026-07-10)

### 问题修复

- make curve downloads transactional (#6)

## [0.3.8](https://github.com/kevinalliswell/yudian-ai-tool/releases/tag/v0.3.8) (2026-07-10)

### 性能优化

- 组件改用细粒度 selector，避免实时读数触发全量重渲染 (#4)

## [0.3.7](https://github.com/kevinalliswell/yudian-ai-tool/releases/tag/v0.3.7) (2026-07-04)

### 问题修复

- 修复错误传递崩溃、UI 错误显示与启动竞态 (#2)

## [0.3.6](https://github.com/kevinalliswell/yudian-ai-tool/releases/tag/v0.3.6) (2026-07-04)

### 问题修复

- keep local unsigned builds simple

## [0.3.5](https://github.com/kevinalliswell/yudian-ai-tool/releases/tag/v0.3.5) (2026-07-04)

### 问题修复

- upload updater manifest

## [0.3.4](https://github.com/kevinalliswell/yudian-ai-tool/releases/tag/v0.3.4) (2026-07-04)

### 问题修复

- enable updater artifacts

## [0.3.3](https://github.com/kevinalliswell/yudian-ai-tool/releases/tag/v0.3.3) (2026-07-04)

### 问题修复

- configure updater signing key

## [0.3.2](https://github.com/kevinalliswell/yudian-ai-tool/releases/tag/v0.3.2) (2026-07-04)

### 问题修复

- allow unsigned macos release builds

## [0.3.1](https://github.com/kevinalliswell/yudian-ai-tool/releases/tag/v0.3.1) (2026-07-04)

### 问题修复

- install macos universal rust targets

## [0.3.0](https://github.com/kevinalliswell/yudian-ai-tool/releases/tag/v0.3.0) (2026-07-04)

### 新功能

- scaffold yudian ai desktop tool

### 问题修复

- make release checkout use pushed commit

## 0.1.0 – 0.2.0 (2026-07-04)

- Tauri v2 + React/TypeScript + Rust 初始实现（这两个版本未打 tag）。
