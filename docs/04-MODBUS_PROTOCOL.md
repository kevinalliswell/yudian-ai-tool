# 04 · Modbus 通讯协议

传输：Modbus RTU，读保持寄存器 0x03，写单寄存器 0x06。

串口默认：9600、8N2、超时约 300ms。从站地址 1-80。

## 寄存器表

| 代号     | 地址 | 含义                        |
| -------- | ---: | --------------------------- |
| SP1      | 0x00 | 给定值                      |
| P        | 0x07 | 比例带                      |
| I        | 0x08 | 积分时间秒                  |
| d        | 0x09 | 微分时间，按 0.1 秒读写对称 |
| dPt      | 0x0C | 小数点位置                  |
| MODEL    | 0x15 | 型号                        |
| MV       | 0x1A | 手动输出                    |
| Srun     | 0x1B | run=0, StoP=1, HoLd=2       |
| Pno      | 0x2B | 程序段数                    |
| PV       | 0x4A | 测量值                      |
| SV       | 0x4B | 给定值回显                  |
| MV_READ  | 0x4C | 输出值，raw/256             |
| SP_START | 0x50 | 曲线段起始地址              |

MODEL：5160 AI-516，5167 AI-516P，5180 AI-518，5187 AI-518P，否则 `未知型号(0xXXXX)`。

## 有符号与哨兵

寄存器按 16 位补码解释：`raw < 32768 ? raw : raw - 65536`。读回 `32512..=32767`（高字节 127）表示无效/保留参数或无数据，返回 `None`。

## dPt

读取失败必须兜底 `decimal_point=1, scale_factor=1`。

```
raw_dpt >= 128 => scale_factor=10, decimal_point=raw_dpt-128
raw_dpt < 128  => scale_factor=1,  decimal_point=raw_dpt
```

令 `f = 10^decimal_point`。

- 读：`value = signed_raw / scale_factor / f`
- 写：`raw = round(actual * f) * scale_factor`

适用：SP1、PV、SV、P。I、MV_READ、段时间不适用。

## 曲线段

先写 Pno。第 i 段：

- 温度：`0x50 + i*2`，按温度换算。
- 时间：`0x50 + i*2 + 1`，整数分钟。

段间节流 50ms，成功失败都执行。

## encode_i16

所有写入统一经 `encode_i16`：先校验 `-32768..=32000`（协议规定参数最大设置值 32000，且 32001 以上会与无效标志或负数区间重叠，读回值与写入值不一致），再按 16 位补码编码。禁止越界截断。

## ValidationLimits

温度 -200~~1800 ℃；P 0~~9999.9；I 0~~9999；D 0~~999.9；段数 1~~50；段时间 0~~3200；从站地址 1~~80；刷新下限 200ms。

限值唯一来源为 Rust `ValidationLimits`，前端 mock 快照 `src/mocks/snapshots/normal.json` 由 Rust 测试校验一致。按手册核定的 P/D/段数/段时间单位等修正见路线图 #26（1.7）。
