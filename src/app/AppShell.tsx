import { useEffect, useMemo, useRef, useState } from "react";
import {
  Activity,
  Cable,
  Download,
  ListChecks,
  Play,
  Plus,
  RefreshCw,
  Route,
  Save,
  Square,
  Trash2,
  Upload,
} from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { zhCN } from "@/i18n/zh-CN";
import { api, runtimeLabel } from "@/lib/api";
import { loadCurvePresets, saveCurvePresets } from "@/lib/curveStorage";
import type { CurvePreset, PidValues, RunStatus, Segment } from "@/lib/types";
import {
  createCurveSchema,
  createPidSchema,
  createSlaveAddressSchema,
  createTemperatureSchema,
} from "@/lib/validation";
import { useShallow } from "zustand/react/shallow";

import { type ShellSection, useAppStore } from "@/stores/appStore";
import { useDeviceStore } from "@/stores/deviceStore";

const sectionIcons = {
  connection: Cable,
  monitor: Activity,
  parameters: ListChecks,
  curves: Route,
} satisfies Record<ShellSection, typeof Cable>;

const sections = Object.keys(sectionIcons) as ShellSection[];

export function AppShell() {
  const activeSection = useAppStore((state) => state.activeSection);
  const setActiveSection = useAppStore((state) => state.setActiveSection);
  // Subscribe only to what the shell renders so live readings (pushed ~1×/s)
  // don't re-render the shell and its active panel.
  const connected = useDeviceStore((state) => state.deviceInfo.connected);
  const modelName = useDeviceStore((state) => state.deviceInfo.modelName);
  const error = useDeviceStore((state) => state.error);
  const limits = useDeviceStore((state) => state.limits);
  const presets = useDeviceStore((state) => state.presets);
  const hydrated = useRef(false);

  useEffect(() => {
    let mounted = true;
    const unlisteners: Array<() => void> = [];
    const actions = useDeviceStore.getState();

    async function boot() {
      const [limits, ports, info] = await Promise.all([
        api.getValidationLimits(),
        api.listSerialPorts().catch(() => []),
        api.getDeviceInfo().catch(() => actions.deviceInfo),
      ]);
      const presets = await loadCurvePresets(limits);
      if (!mounted) return;
      actions.setLimits(limits);
      actions.setPorts(ports);
      actions.setPresets(presets);
      actions.setDeviceInfo(info);
      // Only persist presets once the stored ones are loaded, otherwise the
      // mount-time save effect races boot and can overwrite them with [].
      hydrated.current = true;
      unlisteners.push(await api.onReading(actions.pushReading));
      unlisteners.push(
        await api.onStatus((event) => {
          // The status event only carries connected/model, so merge with the
          // current device info to keep the decimal point / scale / model code
          // that connect() resolved instead of resetting them to defaults.
          const current = useDeviceStore.getState().deviceInfo;
          actions.setDeviceInfo(
            event.connected
              ? { ...current, connected: true, modelName: event.model ?? current.modelName }
              : { connected: false, decimalPoint: 1, scaleFactor: 1 },
          );
        }),
      );
      unlisteners.push(await api.onError(actions.setBackendError));
    }

    boot().catch((error) => actions.setError(readableError(error)));
    return () => {
      mounted = false;
      unlisteners.forEach((unlisten) => unlisten());
      void api.stopMonitoring();
    };
  }, []);

  useEffect(() => {
    if (!hydrated.current || !limits) return;
    void saveCurvePresets(presets, limits);
  }, [limits, presets]);

  const Icon = sectionIcons[activeSection];

  return (
    <main className="min-h-screen bg-background">
      <div className="mx-auto flex min-h-screen max-w-7xl flex-col px-5 py-5">
        <header className="flex flex-wrap items-center justify-between gap-4 border-b pb-4">
          <div>
            <h1 className="text-2xl font-semibold tracking-normal">{zhCN.appName}</h1>
            <p className="mt-1 text-sm text-muted-foreground">{zhCN.subtitle}</p>
          </div>
          <div className="flex items-center gap-2">
            <Badge variant={connected ? "default" : "outline"}>
              {connected ? modelName || "已连接" : "未连接"}
            </Badge>
            <Badge variant="secondary">{runtimeLabel}</Badge>
          </div>
        </header>

        <section className="grid flex-1 gap-5 py-5 lg:grid-cols-[230px_1fr]">
          <nav className="flex gap-2 overflow-x-auto lg:flex-col lg:overflow-visible">
            {sections.map((section) => {
              const NavIcon = sectionIcons[section];
              const selected = activeSection === section;
              return (
                <Button
                  key={section}
                  className="shrink-0 justify-start gap-2"
                  variant={selected ? "default" : "ghost"}
                  onClick={() => setActiveSection(section)}
                >
                  <NavIcon className="h-4 w-4" aria-hidden="true" />
                  {zhCN.sections[section]}
                </Button>
              );
            })}
          </nav>

          <section className="min-w-0 rounded-lg border bg-card p-5 text-card-foreground shadow-sm">
            <div className="mb-5 flex items-center justify-between gap-4">
              <div className="flex items-center gap-2">
                <Icon className="h-5 w-5 text-primary" aria-hidden="true" />
                <h2 className="text-xl font-semibold tracking-normal">
                  {zhCN.sections[activeSection]}
                </h2>
              </div>
              {error ? <Badge variant="outline">{error}</Badge> : null}
            </div>

            {activeSection === "connection" && <ConnectionPanel />}
            {activeSection === "monitor" && <MonitorPanel />}
            {activeSection === "parameters" && <ParametersPanel />}
            {activeSection === "curves" && <CurvesPanel />}
          </section>
        </section>
      </div>
    </main>
  );
}

function ConnectionPanel() {
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
      return;
    }
    setBusy(true);
    try {
      const info = await api.connect(store.connectionConfig);
      store.setDeviceInfo(info);
      store.setError(undefined);
      await api.startMonitoring(1000);
    } catch (error) {
      store.setError(readableError(error));
    } finally {
      setBusy(false);
    }
  }

  async function disconnect() {
    setBusy(true);
    try {
      await api.disconnect();
      store.resetConnectionData();
    } catch (error) {
      store.setError(readableError(error));
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
        <Row label="型号" value={deviceInfo.modelName || "--"} />
        <Row label="小数点" value={`${deviceInfo.decimalPoint}`} />
        <Row label="缩放" value={`${deviceInfo.scaleFactor}`} />
        <Row label="最近读数" value={formatValue(latestReading?.pv, " ℃")} />
      </div>
    </div>
  );
}

function MonitorPanel() {
  const latestReading = useDeviceStore((state) => state.latestReading);
  const readings = useDeviceStore((state) => state.readings);
  const deviceInfo = useDeviceStore((state) => state.deviceInfo);
  return (
    <div className="grid gap-5">
      <div className="grid gap-3 md:grid-cols-3">
        <Metric label="PV 测量值" value={formatValue(latestReading?.pv, " ℃")} />
        <Metric label="SV 给定值" value={formatValue(latestReading?.sv, " ℃")} />
        <Metric label="MV 输出值" value={formatValue(latestReading?.mv, " %")} />
      </div>
      <div className="rounded-md border bg-background p-4">
        <div className="mb-3 flex items-center justify-between">
          <p className="font-medium">PV 时序</p>
          <Badge variant="outline">{deviceInfo.connected ? `${readings.length} 点` : "--"}</Badge>
        </div>
        <PvChart readings={readings} />
      </div>
    </div>
  );
}

function ParametersPanel() {
  const store = useDeviceStore(
    useShallow((state) => ({
      limits: state.limits,
      deviceInfo: state.deviceInfo,
      pid: state.pid,
      setpoint: state.setpoint,
      setPid: state.setPid,
      setSetpoint: state.setSetpoint,
      setError: state.setError,
    })),
  );
  const [pidDraft, setPidDraft] = useState<PidValues>(store.pid);
  const [setpointDraft, setSetpointDraft] = useState(store.setpoint);

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
    if (!createTemperatureSchema(store.limits).safeParse(setpointDraft).success) {
      store.setError("给定值超出范围");
      return;
    }
    try {
      await api.writeSetpoint(setpointDraft);
      store.setSetpoint(setpointDraft);
      store.setError(undefined);
    } catch (error) {
      store.setError(readableError(error));
    }
  }

  async function writePid() {
    if (!store.limits) return;
    if (!createPidSchema(store.limits).safeParse(pidDraft).success) {
      store.setError("PID 参数超出范围");
      return;
    }
    try {
      await api.writePid(pidDraft);
      store.setPid(pidDraft);
      store.setError(undefined);
    } catch (error) {
      store.setError(readableError(error));
    }
  }

  async function setRunStatus(status: RunStatus) {
    try {
      await api.setRunStatus(status);
      store.setError(undefined);
    } catch (error) {
      store.setError(readableError(error));
    }
  }

  return (
    <div className="grid gap-5 xl:grid-cols-2">
      <div className="rounded-md border bg-background p-4">
        <p className="mb-3 font-medium">给定值 SP1</p>
        <div className="flex gap-2">
          <input
            className="h-10 min-w-0 flex-1 rounded-md border bg-background px-3"
            type="number"
            value={setpointDraft}
            disabled={!store.deviceInfo.connected}
            onChange={(event) => setSetpointDraft(Number(event.target.value))}
          />
          <Button onClick={writeSetpoint} disabled={!store.deviceInfo.connected}>
            设置
          </Button>
        </div>
      </div>
      <div className="rounded-md border bg-background p-4">
        <div className="mb-3 flex items-center justify-between">
          <p className="font-medium">PID 参数</p>
          <Button
            variant="outline"
            size="sm"
            onClick={readPid}
            disabled={!store.deviceInfo.connected}
          >
            读取
          </Button>
        </div>
        <div className="grid gap-3 sm:grid-cols-3">
          {(["p", "i", "d"] as const).map((key) => (
            <label key={key} className="grid gap-1 text-sm uppercase">
              {key}
              <input
                className="h-10 rounded-md border bg-background px-3"
                type="number"
                value={pidDraft[key]}
                disabled={!store.deviceInfo.connected}
                onChange={(event) =>
                  setPidDraft({ ...pidDraft, [key]: Number(event.target.value) })
                }
              />
            </label>
          ))}
        </div>
        <Button className="mt-3" onClick={writePid} disabled={!store.deviceInfo.connected}>
          写入 PID
        </Button>
      </div>
      <div className="rounded-md border bg-background p-4 xl:col-span-2">
        <p className="mb-3 font-medium">运行状态</p>
        <div className="flex flex-wrap gap-2">
          <Button onClick={() => setRunStatus("run")} disabled={!store.deviceInfo.connected}>
            <Play className="mr-2 h-4 w-4" />
            运行
          </Button>
          <Button
            variant="secondary"
            onClick={() => setRunStatus("hold")}
            disabled={!store.deviceInfo.connected}
          >
            保持
          </Button>
          <Button
            variant="outline"
            onClick={() => setRunStatus("stop")}
            disabled={!store.deviceInfo.connected}
          >
            <Square className="mr-2 h-4 w-4" />
            停止
          </Button>
        </div>
      </div>
    </div>
  );
}

function CurvesPanel() {
  const store = useDeviceStore(
    useShallow((state) => ({
      limits: state.limits,
      deviceInfo: state.deviceInfo,
      curve: state.curve,
      presets: state.presets,
      setCurve: state.setCurve,
      setPresets: state.setPresets,
      setError: state.setError,
    })),
  );
  const totalMinutes = useMemo(
    () => store.curve.reduce((sum, segment) => sum + segment.minutes, 0),
    [store.curve],
  );

  function updateSegment(index: number, patch: Partial<Segment>) {
    store.setCurve(
      store.curve.map((segment, current) =>
        current === index ? { ...segment, ...patch } : segment,
      ),
    );
  }

  function addSegment() {
    store.setCurve([...store.curve, { temperature: 100, minutes: 10 }]);
  }

  function removeSegment(index: number) {
    store.setCurve(store.curve.filter((_, current) => current !== index));
  }

  async function uploadCurve() {
    try {
      store.setCurve(await api.uploadCurve());
      store.setError(undefined);
    } catch (error) {
      store.setError(readableError(error));
    }
  }

  async function downloadCurve() {
    if (!store.limits) return;
    if (!createCurveSchema(store.limits).safeParse(store.curve).success) {
      store.setError("曲线段超出范围");
      return;
    }
    try {
      await api.downloadCurve(store.curve);
      store.setError(undefined);
    } catch (error) {
      store.setError(readableError(error));
    }
  }

  function savePreset() {
    const preset: CurvePreset = {
      id: crypto.randomUUID(),
      name: `曲线 ${store.presets.length + 1}`,
      description: `${store.curve.length} 段，${totalMinutes} 分钟`,
      segments: store.curve.map((segment) => ({ ...segment })),
    };
    store.setPresets([preset, ...store.presets]);
  }

  return (
    <div className="grid gap-5 xl:grid-cols-[280px_1fr]">
      <div className="rounded-md border bg-background p-4">
        <div className="mb-3 flex items-center justify-between">
          <p className="font-medium">曲线预设</p>
          <Button size="icon" variant="outline" onClick={savePreset}>
            <Save className="h-4 w-4" />
          </Button>
        </div>
        <div className="grid gap-2">
          {store.presets.map((preset) => (
            <button
              key={preset.id}
              className="rounded-md border p-3 text-left text-sm hover:bg-muted"
              onClick={() => store.setCurve(preset.segments.map((segment) => ({ ...segment })))}
            >
              <span className="block font-medium">{preset.name}</span>
              <span className="text-muted-foreground">{preset.description}</span>
            </button>
          ))}
        </div>
      </div>

      <div className="rounded-md border bg-background p-4">
        <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
          <div>
            <p className="font-medium">段编辑</p>
            <p className="text-sm text-muted-foreground">
              {store.curve.length} 段 · {totalMinutes} 分钟 · {(totalMinutes / 60).toFixed(1)} 小时
            </p>
          </div>
          <div className="flex gap-2">
            <Button variant="outline" onClick={uploadCurve} disabled={!store.deviceInfo.connected}>
              <Upload className="mr-2 h-4 w-4" />
              上传
            </Button>
            <Button onClick={downloadCurve} disabled={!store.deviceInfo.connected}>
              <Download className="mr-2 h-4 w-4" />
              下载
            </Button>
          </div>
        </div>
        <div className="grid gap-2">
          {store.curve.map((segment, index) => (
            <div
              key={index}
              className="grid gap-2 rounded-md border p-3 sm:grid-cols-[1fr_1fr_auto]"
            >
              <input
                className="h-10 rounded-md border bg-background px-3"
                type="number"
                value={segment.temperature}
                onChange={(event) =>
                  updateSegment(index, { temperature: Number(event.target.value) })
                }
              />
              <input
                className="h-10 rounded-md border bg-background px-3"
                type="number"
                value={segment.minutes}
                onChange={(event) => updateSegment(index, { minutes: Number(event.target.value) })}
              />
              <Button variant="outline" size="icon" onClick={() => removeSegment(index)}>
                <Trash2 className="h-4 w-4" />
              </Button>
            </div>
          ))}
        </div>
        <Button className="mt-3" variant="secondary" onClick={addSegment}>
          <Plus className="mr-2 h-4 w-4" />
          增加段
        </Button>
      </div>
    </div>
  );
}

function PvChart({ readings }: { readings: Array<{ pv?: number | null }> }) {
  const points = readings.filter((reading) => typeof reading.pv === "number");
  const values = points.map((reading) => reading.pv as number);
  const min = values.length ? Math.min(...values) : 0;
  const max = values.length ? Math.max(...values) : 1;
  const range = Math.max(max - min, 1);
  const path = values
    .map((value, index) => {
      const x = values.length === 1 ? 0 : (index / (values.length - 1)) * 100;
      const y = 100 - ((value - min) / range) * 100;
      return `${index === 0 ? "M" : "L"} ${x.toFixed(2)} ${y.toFixed(2)}`;
    })
    .join(" ");

  return (
    <svg
      className="h-64 w-full rounded-md border bg-card"
      viewBox="0 0 100 100"
      preserveAspectRatio="none"
    >
      <path
        d={path}
        fill="none"
        stroke="hsl(var(--primary))"
        strokeWidth="1.8"
        vectorEffect="non-scaling-stroke"
      />
    </svg>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-md border bg-background p-4">
      <p className="text-sm text-muted-foreground">{label}</p>
      <p className="mt-2 text-3xl font-semibold tracking-normal">{value}</p>
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

function formatValue(value: number | null | undefined, suffix: string) {
  return typeof value === "number" ? `${value.toFixed(1)}${suffix}` : "--";
}

function readableError(error: unknown) {
  if (typeof error === "string") return error;
  if (error && typeof error === "object") {
    const record = error as { message?: unknown; kind?: unknown };
    if (record.message != null && record.message !== "") return String(record.message);
    if (typeof record.kind === "string") return record.kind;
  }
  return String(error);
}
