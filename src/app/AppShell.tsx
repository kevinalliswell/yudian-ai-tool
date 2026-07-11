import { useEffect, useRef } from "react";
import { Activity, AlertCircle, Cable, Gauge, ListChecks, Route } from "lucide-react";

import { Button } from "@/components/ui/button";
import { ConnectionPanel } from "@/features/connection/ConnectionPanel";
import { CurvesPanel } from "@/features/curves/CurvesPanel";
import { MonitorPanel } from "@/features/monitor/MonitorPanel";
import { ParametersPanel } from "@/features/parameters/ParametersPanel";
import { readableError } from "@/features/shared/display";
import { zhCN } from "@/i18n/zh-CN";
import { api, runtimeLabel } from "@/lib/api";
import { loadCurvePresets, saveCurvePresets } from "@/lib/curveStorage";
import { type ShellSection, useAppStore } from "@/stores/appStore";
import { useDeviceStore } from "@/stores/deviceStore";

const sectionIcons = {
  connection: Cable,
  monitor: Activity,
  parameters: ListChecks,
  curves: Route,
} satisfies Record<ShellSection, typeof Cable>;

const sections = Object.keys(sectionIcons) as ShellSection[];

const sectionDescriptions: Record<ShellSection, string> = {
  connection: "配置串口通讯参数，确认设备状态与写入权限。",
  monitor: "查看过程变量、给定值和输出变化。",
  parameters: "同步控制参数，执行写入与运行控制。",
  curves: "编辑、上传并验证温控曲线段。",
};

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
              : { connected: false, writeEnabled: false, decimalPoint: 1, scaleFactor: 1 },
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
    <main className="app-shell">
      <div className="app-frame">
        <header className="topbar">
          <div className="brand-lockup">
            <div className="brand-mark">
              <Gauge className="h-5 w-5" aria-hidden="true" />
            </div>
            <div>
              <p className="eyebrow">YUDIAN / CONTROL SUITE</p>
              <h1 className="brand-title">{zhCN.appName}</h1>
            </div>
          </div>
          <div className="topbar-meta">
            <div className={`status-chip ${connected ? "status-chip-connected" : ""}`}>
              <span className={`status-dot ${connected ? "status-dot-live" : ""}`} />
              <span>{connected ? modelName || "已连接" : "未连接"}</span>
            </div>
            <div className="status-chip">{runtimeLabel}</div>
          </div>
        </header>

        <div className="workspace">
          <aside className="sidebar">
            <p className="sidebar-label">控制台</p>
            <nav className="nav-list" aria-label="主导航">
              {sections.map((section) => {
                const NavIcon = sectionIcons[section];
                const selected = activeSection === section;
                return (
                  <Button
                    key={section}
                    className={`nav-item ${selected ? "nav-item-active" : ""}`}
                    variant="ghost"
                    aria-current={selected ? "page" : undefined}
                    onClick={() => setActiveSection(section)}
                  >
                    <NavIcon className="h-4 w-4" aria-hidden="true" />
                    {zhCN.sections[section]}
                  </Button>
                );
              })}
            </nav>
            <div className="sidebar-footer">
              <strong>AI 温控设备</strong>
              串口控制 · 曲线运行 · 实时采集
            </div>
          </aside>

          <section className="content-area">
            <div className="page-heading">
              <div>
                <p className="page-kicker">OPERATIONS / {zhCN.sections[activeSection]}</p>
                <div className="flex items-center gap-2">
                  <Icon className="mt-2 h-5 w-5 text-primary" aria-hidden="true" />
                  <h2 className="page-title">{zhCN.sections[activeSection]}</h2>
                </div>
                <p className="page-description">{sectionDescriptions[activeSection]}</p>
              </div>
              {error ? (
                <div className="error-banner" role="alert">
                  <AlertCircle className="mt-0.5 h-4 w-4 shrink-0" aria-hidden="true" />
                  <span>{error}</span>
                </div>
              ) : null}
            </div>

            <div className="panel-content">
              {activeSection === "connection" && <ConnectionPanel />}
              {activeSection === "monitor" && <MonitorPanel />}
              {activeSection === "parameters" && <ParametersPanel />}
              {activeSection === "curves" && <CurvesPanel />}
            </div>
          </section>
        </div>
      </div>
    </main>
  );
}
