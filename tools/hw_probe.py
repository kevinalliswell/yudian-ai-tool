"""Read-only hardware probe for Yudian AI-5xx controllers over Modbus RTU.

Answers the register-map questions the vendor manual leaves open (PV/SV/MV
addresses, P scaling, program time resolution, standard vs. compatible
Modbus mode) by reading registers only. This script sends function code
0x03 exclusively; it contains no write path and cannot change the device.

Usage:
    python tools/hw_probe.py COM3 --addr 1
    python tools/hw_probe.py COM3 --addr 1 --baud 9600 --stopbits 2 --parity N

Requires pyserial (`pip install pyserial`).
"""

from __future__ import annotations

import argparse
import struct
import sys
import time

import serial

SENTINEL_MIN = 32512  # protocol: invalid/reserved codes return high byte 127

REGISTERS = [
    (0x15, "MODEL", "型号特征字: 5160/5167/5180/5187"),
    (0x0C, "dPt", "小数点: 0-3, >=128 表示数值需再 /10"),
    (0x00, "SP1", "给定值, 单位同 PV"),
    (0x07, "P", "比例带, 单位同 PV（验证是否受 dPt 影响）"),
    (0x08, "I", "积分时间, 秒"),
    (0x09, "d", "微分时间, 0.1 秒"),
    (0x19, "Loc", "参数封锁（V9.16+ 可限制通讯写入）"),
    (0x1B, "Srun", "0=run 1=StoP 2=HoLd"),
    (0x1E, "SPL", "给定值下限"),
    (0x1F, "SPH", "给定值上限"),
    (0x2B, "Pno", "程序段数 0-30（仅 P 型）"),
    (0x2D, "PAF", "程序参数: C 位=小时, G 位=秒"),
    (0x2E, "STEP", "当前程序段号"),
    (0x2F, "t_run", "已运行时间, 0.1 分/小时"),
    (0x4A, "PV?", "代码假定的测量值地址"),
    (0x4B, "SV?", "代码假定的给定值回显地址"),
    (0x4C, "MV?", "代码假定的输出值地址 (raw/256?)"),
    (0x50, "SP 1", "第 1 段给定值"),
    (0x51, "t 1", "第 1 段时间（验证是否 x10）"),
    (0x52, "SP 2", "第 2 段给定值"),
    (0x53, "t 2", "第 2 段时间"),
]


def crc16(frame: bytes) -> int:
    crc = 0xFFFF
    for byte in frame:
        crc ^= byte
        for _ in range(8):
            crc = (crc >> 1) ^ 0xA001 if crc & 1 else crc >> 1
    return crc


def read_holding(port: serial.Serial, slave: int, addr: int, count: int) -> list[int]:
    request = struct.pack(">BBHH", slave, 0x03, addr, count)
    request += struct.pack("<H", crc16(request))
    port.reset_input_buffer()
    port.write(request)

    expected = 5 + 2 * count
    response = port.read(expected)
    if len(response) >= 5 and response[1] & 0x80:
        raise IOError(f"exception code {response[2]}")
    if len(response) != expected:
        raise IOError(f"short response: {len(response)}/{expected} bytes {response.hex(' ')}")
    if struct.unpack("<H", response[-2:])[0] != crc16(response[:-2]):
        raise IOError(f"CRC mismatch: {response.hex(' ')}")
    return list(struct.unpack(f">{count}H", response[3:-2]))


def signed(raw: int) -> int:
    return raw - 0x10000 if raw >= 0x8000 else raw


def describe(raw: int) -> str:
    if SENTINEL_MIN <= raw <= 0x7FFF:
        return "INVALID/RESERVED"
    return str(signed(raw))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    parser.add_argument("port")
    parser.add_argument("--addr", type=int, default=1)
    parser.add_argument("--baud", type=int, default=9600)
    parser.add_argument("--parity", choices=["N", "E"], default="N")
    parser.add_argument("--stopbits", type=int, choices=[1, 2], default=2)
    args = parser.parse_args()

    with serial.Serial(
        args.port,
        args.baud,
        bytesize=8,
        parity=args.parity,
        stopbits=args.stopbits,
        timeout=0.5,
    ) as port:
        print(f"# {args.port} addr={args.addr} {args.baud} 8{args.parity}{args.stopbits}\n")
        print(f"{'addr':>6}  {'name':<6} {'raw':>6} {'signed':>16}  note")
        for addr, name, note in REGISTERS:
            try:
                raw = read_holding(port, args.addr, addr, 1)[0]
                print(f"0x{addr:02X}    {name:<6} {raw:>6} {describe(raw):>16}  {note}")
            except IOError as err:
                print(f"0x{addr:02X}    {name:<6} {'--':>6} {'ERROR':>16}  {err}")
            time.sleep(0.05)

        # Standard Modbus (AFC=0) accepts multi-word reads; compatible mode
        # (AFC=2) only accepts exactly 4 words. The app relies on the former.
        print("\n# multi-register read support")
        for addr, count in [(0x4A, 3), (0x07, 3), (0x50, 2), (0x00, 4)]:
            try:
                values = read_holding(port, args.addr, addr, count)
                print(f"0x{addr:02X} x{count}: OK {[describe(v) for v in values]}")
            except IOError as err:
                print(f"0x{addr:02X} x{count}: FAILED {err}")
            time.sleep(0.05)
    return 0


if __name__ == "__main__":
    sys.exit(main())
