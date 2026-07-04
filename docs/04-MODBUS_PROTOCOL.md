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

寄存器按 16 位补码解释：`raw < 32768 ? raw : raw - 65536`。`32767` 表示无数据，返回 `None`。

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

## to_uint16

必须先校验 `-32768..=65535`，再对负数加 65536。禁止越界截断。

## ValidationLimits

温度 -200~~1800 ℃；P 0~~9999.9；I 0~~9999；D 0~~999.9；段数 1~~50；从站地址 1~~80；刷新下限 200ms。
