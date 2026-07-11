import { useState } from "react";
import { Play, Square } from "lucide-react";
import { useShallow } from "zustand/react/shallow";

import { Button } from "@/components/ui/button";
import { zhCN } from "@/i18n/zh-CN";
import { api } from "@/lib/api";
import { recordAuditEvent, rollbackStatusFromError } from "@/lib/auditLog";
import type { PidValues, RunStatus } from "@/lib/types";
import { createPidSchema, createTemperatureSchema } from "@/lib/validation";
import { useDeviceStore } from "@/stores/deviceStore";

import { readableError } from "@/features/shared/display";

export function ParametersPanel() {
  const store = useDeviceStore(
    useShallow((state) => ({
      limits: state.limits,
      deviceInfo: state.deviceInfo,
      parameterSync: state.parameterSync,
      connectionConfig: state.connectionConfig,
      pid: state.pid,
      setpoint: state.setpoint,
      curve: state.curve,
      setPid: state.setPid,
      setSetpoint: state.setSetpoint,
      setParameterSync: state.setParameterSync,
      setError: state.setError,
    })),
  );
  const [pidDraft, setPidDraft] = useState<PidValues>(store.pid);
  const [setpointDraft, setSetpointDraft] = useState(store.setpoint);

  async function readParameters() {
    store.setParameterSync("syncing");
    try {
      const [setpoint, pid] = await Promise.all([api.readSetpoint(), api.readPid()]);
      store.setSetpoint(setpoint);
      store.setPid(pid);
      setSetpointDraft(setpoint);
      setPidDraft(pid);
      store.setParameterSync("synced");
      store.setError(undefined);
    } catch (error) {
      store.setParameterSync("failed");
      store.setError(`参数同步失败：${readableError(error)}`);
    }
  }

  async function readPid() {
    try {
      const values = await api.readPid();
      store.setPid(values);
      setPidDraft(values);
      store.setError(undefined);
    } catch (error) {
      store.setError(readableError(error));
    }
  }

  async function writeSetpoint() {
    if (!store.limits) return;
    const before = store.setpoint;
    if (!createTemperatureSchema(store.limits).safeParse(setpointDraft).success) {
      store.setError("给定值超出范围");
      await recordAuditEvent({
        action: "write_setpoint",
        outcome: "rejected",
        details: {
          connection: store.connectionConfig,
          device: store.deviceInfo,
          before,
          attempted: setpointDraft,
          reason: "value_out_of_range",
        },
      });
      return;
    }
    try {
      if (store.parameterSync !== "synced") {
        store.setError("参数尚未同步，不能写入");
        await recordAuditEvent({
          action: "write_setpoint",
          outcome: "rejected",
          details: {
            connection: store.connectionConfig,
            device: store.deviceInfo,
            before,
            attempted: setpointDraft,
            reason: "parameters_not_synced",
          },
        });
        return;
      }
      if (!store.deviceInfo.writeEnabled) {
        store.setError("设备为只读模式，DPT 读取失败，不能写入");
        await recordAuditEvent({
          action: "write_setpoint",
          outcome: "rejected",
          details: {
            connection: store.connectionConfig,
            device: store.deviceInfo,
            before,
            attempted: setpointDraft,
            reason: "device_read_only",
          },
        });
        return;
      }
      await api.writeSetpoint(setpointDraft);
      store.setSetpoint(setpointDraft);
      store.setError(undefined);
      await recordAuditEvent({
        action: "write_setpoint",
        outcome: "success",
        details: {
          connection: store.connectionConfig,
          device: store.deviceInfo,
          before,
          after: setpointDraft,
          rollback: "not_applicable",
        },
      });
    } catch (error) {
      const message = readableError(error);
      store.setError(message);
      await recordAuditEvent({
        action: "write_setpoint",
        outcome: "failure",
        details: {
          connection: store.connectionConfig,
          device: store.deviceInfo,
          before,
          attempted: setpointDraft,
          error: message,
          rollback: "not_applicable",
        },
      });
    }
  }

  async function writePid() {
    if (!store.limits) return;
    const before = { ...store.pid };
    if (!createPidSchema(store.limits).safeParse(pidDraft).success) {
      store.setError("PID 参数超出范围");
      await recordAuditEvent({
        action: "write_pid",
        outcome: "rejected",
        details: {
          connection: store.connectionConfig,
          device: store.deviceInfo,
          before,
          attempted: pidDraft,
          reason: "value_out_of_range",
        },
      });
      return;
    }
    try {
      if (store.parameterSync !== "synced") {
        store.setError("参数尚未同步，不能写入");
        await recordAuditEvent({
          action: "write_pid",
          outcome: "rejected",
          details: {
            connection: store.connectionConfig,
            device: store.deviceInfo,
            before,
            attempted: pidDraft,
            reason: "parameters_not_synced",
          },
        });
        return;
      }
      if (!store.deviceInfo.writeEnabled) {
        store.setError("设备为只读模式，DPT 读取失败，不能写入");
        await recordAuditEvent({
          action: "write_pid",
          outcome: "rejected",
          details: {
            connection: store.connectionConfig,
            device: store.deviceInfo,
            before,
            attempted: pidDraft,
            reason: "device_read_only",
          },
        });
        return;
      }
      await api.writePid(pidDraft);
      store.setPid(pidDraft);
      store.setError(undefined);
      await recordAuditEvent({
        action: "write_pid",
        outcome: "success",
        details: {
          connection: store.connectionConfig,
          device: store.deviceInfo,
          before,
          after: pidDraft,
          rollback: "not_applicable",
        },
      });
    } catch (error) {
      const message = readableError(error);
      store.setError(message);
      await recordAuditEvent({
        action: "write_pid",
        outcome: "failure",
        details: {
          connection: store.connectionConfig,
          device: store.deviceInfo,
          before,
          attempted: pidDraft,
          error: message,
          rollback: rollbackStatusFromError(error),
        },
      });
    }
  }

  async function setRunStatus(status: RunStatus) {
    const details = {
      connection: store.connectionConfig,
      device: store.deviceInfo,
      status,
    };
    if (status === "run") {
      const totalMinutes = store.curve.reduce((sum, segment) => sum + segment.minutes, 0);
      const confirmed = window.confirm(
        `${zhCN.runConfirmation.title}\n${zhCN.runConfirmation.summary(store.curve.length, totalMinutes)}`,
      );
      if (!confirmed) {
        await recordAuditEvent({
          action: "run_status",
          outcome: "rejected",
          details: { ...details, reason: "user_cancelled" },
        });
        return;
      }
    }
    try {
      await api.setRunStatus(status);
      store.setError(undefined);
      await recordAuditEvent({ action: "run_status", outcome: "success", details });
    } catch (error) {
      const message = readableError(error);
      store.setError(message);
      await recordAuditEvent({
        action: "run_status",
        outcome: "failure",
        details: { ...details, error: message },
      });
    }
  }

  return (
    <div className="parameters-layout">
      <div className="sync-bar">
        <div>
          <p className="surface-kicker">参数同步</p>
          <p className="sync-state">
            <span className="status-dot status-dot-live" />
            <strong>
              {store.parameterSync === "synced"
                ? "已读取当前设备值"
                : store.parameterSync === "syncing"
                  ? "正在读取设备值"
                  : store.parameterSync === "failed"
                    ? "读取失败，请重试"
                    : "尚未读取设备值"}
            </strong>
          </p>
        </div>
        <Button
          variant="outline"
          onClick={readParameters}
          disabled={!store.deviceInfo.connected || store.parameterSync === "syncing"}
        >
          读取当前值
        </Button>
      </div>
      <section className="surface parameter-surface">
        <div className="surface-header">
          <div>
            <p className="surface-kicker">SETPOINT</p>
            <h3 className="surface-title">给定值 SP1</h3>
          </div>
          <span className="section-number">03</span>
        </div>
        <div className="action-row">
          <input
            className="control-input"
            aria-label="给定值 SP1"
            type="number"
            value={store.parameterSync === "synced" ? setpointDraft : ""}
            disabled={
              !store.deviceInfo.connected ||
              !store.deviceInfo.writeEnabled ||
              store.parameterSync !== "synced"
            }
            onChange={(event) => setSetpointDraft(Number(event.target.value))}
          />
          <Button
            onClick={writeSetpoint}
            disabled={
              !store.deviceInfo.connected ||
              !store.deviceInfo.writeEnabled ||
              store.parameterSync !== "synced"
            }
          >
            设置
          </Button>
        </div>
      </section>
      <section className="surface parameter-surface">
        <div className="surface-header">
          <div>
            <p className="surface-kicker">CONTROL LOOP</p>
            <h3 className="surface-title">PID 参数</h3>
          </div>
          <span className="section-number">04</span>
        </div>
        <div className="action-row parameter-heading">
          <span className="surface-subtitle">比例、积分、微分</span>
          <Button
            variant="outline"
            size="sm"
            onClick={readPid}
            disabled={!store.deviceInfo.connected}
          >
            读取
          </Button>
        </div>
        <div className="pid-grid">
          {(["p", "i", "d"] as const).map((key) => (
            <label key={key} className="field">
              {key}
              <input
                className="control-input"
                aria-label={`PID ${key.toUpperCase()}`}
                type="number"
                value={store.parameterSync === "synced" ? pidDraft[key] : ""}
                disabled={
                  !store.deviceInfo.connected ||
                  !store.deviceInfo.writeEnabled ||
                  store.parameterSync !== "synced"
                }
                onChange={(event) =>
                  setPidDraft({ ...pidDraft, [key]: Number(event.target.value) })
                }
              />
            </label>
          ))}
        </div>
        <Button
          className="mt-3"
          onClick={writePid}
          disabled={
            !store.deviceInfo.connected ||
            !store.deviceInfo.writeEnabled ||
            store.parameterSync !== "synced"
          }
        >
          写入 PID
        </Button>
      </section>
      <section className="surface parameter-surface parameter-surface-wide">
        <div className="surface-header">
          <div>
            <p className="surface-kicker">EXECUTION</p>
            <h3 className="surface-title">运行状态</h3>
          </div>
          <span className="section-number">05</span>
        </div>
        <div className="action-row">
          <Button
            onClick={() => setRunStatus("run")}
            disabled={!store.deviceInfo.connected || !store.deviceInfo.writeEnabled}
          >
            <Play className="mr-2 h-4 w-4" />
            运行
          </Button>
          <Button
            variant="secondary"
            onClick={() => setRunStatus("hold")}
            disabled={!store.deviceInfo.connected || !store.deviceInfo.writeEnabled}
          >
            保持
          </Button>
          <Button
            variant="outline"
            onClick={() => setRunStatus("stop")}
            disabled={!store.deviceInfo.connected || !store.deviceInfo.writeEnabled}
          >
            <Square className="mr-2 h-4 w-4" />
            停止
          </Button>
        </div>
      </section>
    </div>
  );
}
