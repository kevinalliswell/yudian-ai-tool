import { useState } from "react";
import { RefreshCw } from "lucide-react";
import { useShallow } from "zustand/react/shallow";

import { Button } from "@/components/ui/button";
import { api } from "@/lib/api";
import { recordAuditEvent } from "@/lib/auditLog";
import { createSlaveAddressSchema } from "@/lib/validation";
import { useDeviceStore } from "@/stores/deviceStore";

import { formatValue, readableError } from "@/features/shared/display";

export function ConnectionPanel() {
  const store = useDeviceStore(
    useShallow((state) => ({
      limits: state.limits,
      connectionConfig: state.connectionConfig,
      ports: state.ports,
      deviceInfo: state.deviceInfo,
      setPorts: state.setPorts,
      setError: state.setError,
      setConnectionConfig: state.setConnectionConfig,
      setDeviceInfo: state.setDeviceInfo,
      setPid: state.setPid,
      setSetpoint: state.setSetpoint,
      setParameterSync: state.setParameterSync,
      resetConnectionData: state.resetConnectionData,
    })),
  );
  const [busy, setBusy] = useState(false);

  async function refreshPorts() {
    setBusy(true);
    try {
      store.setPorts(await api.listSerialPorts());
      store.setError(undefined);
    } catch (error) {
      store.setError(readableError(error));
    } finally {
      setBusy(false);
    }
  }

  async function connect() {
    if (!store.limits) return;
    const parsed = createSlaveAddressSchema(store.limits).safeParse(
      store.connectionConfig.slaveAddr,
    );
    if (!parsed.success) {
      store.setError("从站地址超出范围");
      await recordAuditEvent({
        action: "connect",
        outcome: "rejected",
        details: {
          connection: { ...store.connectionConfig },
          reason: "slave_address_out_of_range",
        },
      });
      return;
    }
    setBusy(true);
    let parameterSync: "synced" | "failed" = "failed";
    try {
      const info = await api.connect(store.connectionConfig);
      store.setDeviceInfo(info);
      store.setParameterSync("syncing");
      try {
        const [setpoint, pid] = await Promise.all([api.readSetpoint(), api.readPid()]);
        store.setSetpoint(setpoint);
        store.setPid(pid);
        store.setParameterSync("synced");
        parameterSync = "synced";
        store.setError(undefined);
      } catch (error) {
        store.setParameterSync("failed");
        store.setError(`参数同步失败：${readableError(error)}`);
      }
      await api.startMonitoring(1000);
      await recordAuditEvent({
        action: "connect",
        outcome: "success",
        details: {
          connection: { ...store.connectionConfig },
          device: info,
          status: `parameter_sync_${parameterSync}`,
        },
      });
    } catch (error) {
      const message = readableError(error);
      store.setError(message);
      await recordAuditEvent({
        action: "connect",
        outcome: "failure",
        details: {
          connection: { ...store.connectionConfig },
          device: store.deviceInfo,
          error: message,
        },
      });
    } finally {
      setBusy(false);
    }
  }

  async function disconnect() {
    setBusy(true);
    const device = store.deviceInfo;
    const connection = { ...store.connectionConfig };
    try {
      await api.disconnect();
      store.resetConnectionData();
      await recordAuditEvent({
        action: "disconnect",
        outcome: "success",
        details: { connection, device },
      });
    } catch (error) {
      const message = readableError(error);
      store.setError(message);
      await recordAuditEvent({
        action: "disconnect",
        outcome: "failure",
        details: { connection, device, error: message },
      });
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="grid gap-5 xl:grid-cols-[1fr_320px]">
      <div className="grid gap-4 sm:grid-cols-2">
        <label className="grid gap-2 text-sm">
          串口
          <select
            className="h-10 rounded-md border bg-background px-3"
            value={store.connectionConfig.port}
            onChange={(event) => store.setConnectionConfig({ port: event.target.value })}
            disabled={store.deviceInfo.connected}
          >
            {store.ports.map((port) => (
              <option key={port.name} value={port.name}>
                {port.name}
                {port.description ? ` · ${port.description}` : ""}
              </option>
            ))}
          </select>
        </label>
        <label className="grid gap-2 text-sm">
          从站地址
          <input
            className="h-10 rounded-md border bg-background px-3"
            type="number"
            min={store.limits?.slaveAddrMin}
            max={store.limits?.slaveAddrMax}
            value={store.connectionConfig.slaveAddr}
            onChange={(event) =>
              store.setConnectionConfig({ slaveAddr: Number(event.target.value) })
            }
            disabled={store.deviceInfo.connected}
          />
        </label>
        <label className="grid gap-2 text-sm">
          波特率
          <input
            className="h-10 rounded-md border bg-muted px-3 text-muted-foreground"
            value={store.connectionConfig.baudrate}
            readOnly
          />
        </label>
        <div className="flex items-end gap-2">
          <Button
            variant="outline"
            onClick={refreshPorts}
            disabled={busy || store.deviceInfo.connected}
          >
            <RefreshCw className="mr-2 h-4 w-4" />
            刷新
          </Button>
          {store.deviceInfo.connected ? (
            <Button variant="secondary" onClick={disconnect} disabled={busy}>
              断开
            </Button>
          ) : (
            <Button onClick={connect} disabled={busy || !store.connectionConfig.port}>
              连接
            </Button>
          )}
        </div>
      </div>

      <StatusBox />
    </div>
  );
}

function StatusBox() {
  const deviceInfo = useDeviceStore((state) => state.deviceInfo);
  const latestReading = useDeviceStore((state) => state.latestReading);
  return (
    <div className="rounded-md border bg-background p-4">
      <p className="text-sm text-muted-foreground">设备状态</p>
      <div className="mt-3 grid gap-2 text-sm">
        <Row label="连接" value={deviceInfo.connected ? "已连接" : "未连接"} />
        <Row label="写入权限" value={deviceInfo.writeEnabled ? "可写" : "只读"} />
        <Row label="型号" value={deviceInfo.modelName || "--"} />
        <Row label="小数点" value={`${deviceInfo.decimalPoint}`} />
        <Row label="缩放" value={`${deviceInfo.scaleFactor}`} />
        <Row label="最近读数" value={formatValue(latestReading?.pv, " ℃")} />
      </div>
    </div>
  );
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex justify-between gap-4">
      <span className="text-muted-foreground">{label}</span>
      <span className="font-medium">{value}</span>
    </div>
  );
}
