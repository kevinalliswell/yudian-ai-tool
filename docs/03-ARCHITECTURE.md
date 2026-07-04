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
