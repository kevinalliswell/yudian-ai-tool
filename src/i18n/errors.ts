import type { AppError } from "@/lib/types";

import { zhCN } from "./zh-CN";

const LABELS: Record<string, string> = {
  temperature: "温度",
  "PID P": "PID 比例带 P",
  "PID I": "PID 积分时间 I",
  "PID D": "PID 微分时间 D",
  "slave address": "从站地址",
  "segment count": "曲线段数",
  "curve segment count": "曲线段数",
  "segment minutes": "段时间",
  "scaled value": "换算后的数值",
  Pno: "段数 Pno",
};

function describeLabel(label: string) {
  const segment = /^segment (\d+) (temperature|minutes)$/.exec(label);
  if (segment) {
    return `第 ${Number(segment[1]) + 1} 段${segment[2] === "temperature" ? "温度" : "时间"}`;
  }
  return LABELS[label] ?? label;
}

/** Drops the English prefix Rust puts in front of transport errors. */
function detail(message: string | undefined) {
  return (message ?? "").replace(/^(serial|modbus|backend) error: |^invalid data: /, "");
}

const READ_ONLY: Record<string, string> = {
  dptUnavailable: "设备为只读模式：未能读取小数点设置(dPt)",
  unsupportedModel: "设备为只读模式：型号不受支持",
};

const RUN_BLOCKED: Record<string, string> = {
  unsupportedModel: "无法运行：设备型号不受支持",
  curveNotVerified: zhCN.runConfirmation.notVerified,
  invalidPv: "无法运行：测量值 PV 无效或超出温度范围",
  invalidSv: "无法运行：给定值 SV 无效或超出温度范围",
};

const WRITE_OPERATIONS: Record<string, string> = {
  pid: "PID 写入",
  curve: "曲线下载",
};

function describeAppError(error: AppError): string | undefined {
  switch (error.kind) {
    case "notConnected":
      return "设备未连接";
    case "timeout":
      return "设备响应超时，请检查接线、从站地址与波特率";
    case "outOfRange":
      return `${describeLabel(error.label ?? "")} 超出范围：${error.value}（允许 ${error.min} ~ ${error.max}）`;
    case "serial":
      return `串口错误：${detail(error.message)}`;
    case "modbus":
      return `通讯错误：${detail(error.message)}`;
    case "backend":
      return `内部错误：${detail(error.message)}`;
    case "invalidData":
      return `设备返回的数据无效：${detail(error.message)}`;
    case "busy":
      return "设备正在执行写入，请稍后重试";
    case "deviceRunning":
      return "程序运行中，请先暂停(HoLd)或停止后再下载曲线";
    case "outcomeUnknown":
      return "写入结果未知，请稍后重新读取设备当前值";
    case "readOnly":
      return READ_ONLY[error.reason ?? ""];
    case "runBlocked":
      return RUN_BLOCKED[error.reason ?? ""];
    case "writeFailed": {
      const operation = WRITE_OPERATIONS[error.operation ?? ""] ?? "写入";
      return error.rollback === "succeeded"
        ? `${operation}失败，已恢复为原值`
        : `${operation}失败，且恢复原值也失败，请立即检查设备`;
    }
    default:
      return undefined;
  }
}

function isAppError(error: unknown): error is AppError {
  return (
    typeof error === "object" &&
    error !== null &&
    typeof (error as { kind?: unknown }).kind === "string"
  );
}

/**
 * User-facing Chinese text for any thrown value. Backend errors are
 * localized by `kind` and their structured fields; Rust's English `message`
 * is only a fallback for kinds this table does not know yet.
 */
export function describeError(error: unknown): string {
  if (typeof error === "string") return error;
  if (isAppError(error)) {
    return describeAppError(error) ?? error.message ?? error.kind;
  }
  if (error instanceof Error) return error.message;
  if (error && typeof error === "object" && "message" in error) {
    return String((error as { message: unknown }).message);
  }
  return String(error);
}
