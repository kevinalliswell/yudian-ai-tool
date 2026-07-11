import { useEffect, useRef } from "react";
import { Activity, Cable, ListChecks, Route } from "lucide-react";

import { Badge } from "@/components/ui/badge";
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
