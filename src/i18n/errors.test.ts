import { describe, expect, it } from "vitest";

import { zhCN } from "@/i18n/zh-CN";

import { describeError } from "./errors";

describe("describeError", () => {
  it("localizes structured backend errors by kind", () => {
    expect(describeError({ kind: "notConnected", message: "device is not connected" })).toBe(
      "设备未连接",
    );
    expect(describeError({ kind: "busy", message: "device is busy" })).toBe(
      "设备正在执行写入，请稍后重试",
    );
    expect(describeError({ kind: "readOnly", message: "…", reason: "dptUnavailable" })).toContain(
      "未能读取小数点设置",
    );
    expect(describeError({ kind: "runBlocked", message: "…", reason: "curveNotVerified" })).toBe(
      zhCN.runConfirmation.notVerified,
    );
  });

  it("names the parameter and the allowed range for out-of-range values", () => {
    expect(
      describeError({
        kind: "outOfRange",
        message: "PID D out of range: 400, expected 0..=320",
        label: "PID D",
        value: 400,
        min: 0,
        max: 320,
      }),
    ).toBe("PID 微分时间 D 超出范围：400（允许 0 ~ 320）");
    expect(
      describeError({
        kind: "outOfRange",
        message: "…",
        label: "segment 2 temperature",
        value: 1900,
        min: -200,
        max: 1800,
      }),
    ).toBe("第 3 段温度 超出范围：1900（允许 -200 ~ 1800）");
  });

  it("states whether a failed write was rolled back", () => {
    expect(
      describeError({
        kind: "writeFailed",
        message: "…",
        operation: "curve",
        rollback: "succeeded",
      }),
    ).toBe("曲线下载失败，已恢复为原值");
    expect(
      describeError({ kind: "writeFailed", message: "…", operation: "pid", rollback: "failed" }),
    ).toBe("PID 写入失败，且恢复原值也失败，请立即检查设备");
  });

  it("keeps the backend detail for transport errors and unknown kinds", () => {
    expect(describeError({ kind: "serial", message: "serial error: port busy" })).toBe(
      "串口错误：port busy",
    );
    expect(describeError({ kind: "somethingNew", message: "raw text" })).toBe("raw text");
    expect(describeError("plain string")).toBe("plain string");
    expect(describeError(new Error("boom"))).toBe("boom");
  });
});
