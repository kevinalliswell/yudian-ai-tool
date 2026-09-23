# 03 · 架构

```
React UI -> src/lib/api.ts -> Tauri invoke/event -> Rust DeviceActor -> DeviceBackend
```

## DeviceActor

- 串口/Modbus 连接由唯一 Actor 拥有。
- invoke handler 和监控任务只通过 mpsc/oneshot 给 Actor 发命令。
- 读写共用同一队列，天然串行化半双工总线。

## Commands

- `list_serial_ports`
- `connect`
- `disconnect`
- `get_device_info`
- `get_validation_limits`
- `read_pid`
- `write_setpoint`
- `write_pid`
- `set_run_status`
- `upload_curve`
- `download_curve`
- `start_monitoring`
- `stop_monitoring`

## Events

- `device://reading`：`{ pv?, sv?, mv?, ts }`
- `device://status`：`{ connected, model? }`
- `device://error`：`{ scope, message }`

配置与曲线预设不做 invoke，走 store。

## 写入事务

`write_pid` 与 `download_curve` 是多寄存器写入事务（备份 → 写入 → 回读校验 → 失败回滚），必须完整执行：

- 事务先在队列中等待开始（上限 5s）。调用方超时放弃时 Actor 在首次总线操作前跳过该请求，**零写入**。
- 事务开始后不再套外层超时；每个后端调用由 `GuardedBackend` 限时 1s，超时变为普通错误并进入回滚。
- 曾有调用超时的事务结束后重置链路（迟到的应答可能错位）。
- 调用方按最坏情况等待（`transaction_ceiling`）；仍未完成返回 `outcomeUnknown`，前端应重新读取设备值。
- 事务期间其他请求立即返回 `busy`；监控循环遇到 `busy` 只等待下一周期，不计入断线失败。
