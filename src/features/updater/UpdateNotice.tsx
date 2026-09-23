import { useEffect, useState } from "react";

import { Button } from "@/components/ui/button";
import { readableError } from "@/features/shared/display";
import { api } from "@/lib/api";
import { confirmAction } from "@/lib/dialog";
import { checkForUpdate, updatesSupported, type AvailableUpdate } from "@/lib/updater";
import { useDeviceStore } from "@/stores/deviceStore";

type Phase =
  | { name: "idle" }
  | { name: "checking" }
  | { name: "upToDate" }
  | { name: "available"; update: AvailableUpdate }
  | { name: "installing"; progress?: number }
  | { name: "error"; message: string };

export function UpdateNotice() {
  const supported = updatesSupported();
  const connected = useDeviceStore((state) => state.deviceInfo.connected);
  const [phase, setPhase] = useState<Phase>({ name: "idle" });

  // Silent check at startup: being offline must not bother the operator.
  useEffect(() => {
    if (!supported) return;
    let active = true;
    checkForUpdate()
      .then((update) => {
        if (active && update) setPhase({ name: "available", update });
      })
      .catch((error) => console.warn("startup update check failed", error));
    return () => {
      active = false;
    };
  }, [supported]);

  if (!supported) return null;

  async function checkManually() {
    setPhase({ name: "checking" });
    try {
      const update = await checkForUpdate();
      setPhase(update ? { name: "available", update } : { name: "upToDate" });
    } catch (error) {
      setPhase({ name: "error", message: `检查更新失败：${readableError(error)}` });
    }
  }

  async function install(update: AvailableUpdate) {
    const confirmed = await confirmAction(
      `将下载并安装 v${update.version}，完成后应用会自动重启。${
        connected ? "当前设备连接会先断开。" : ""
      }`,
      "安装更新",
    );
    if (!confirmed) return;
    try {
      // Never let an install interrupt a live Modbus session mid-write.
      if (connected) {
        await api.stopMonitoring();
        await api.disconnect();
      }
      setPhase({ name: "installing" });
      await update.install((progress) => setPhase({ name: "installing", progress }));
    } catch (error) {
      setPhase({ name: "error", message: `更新失败：${readableError(error)}` });
    }
  }

  return (
    <div className="flex items-center gap-2 text-sm">
      {phase.name === "available" && (
        <>
          <span className="font-medium text-primary" title={phase.update.notes}>
            发现新版本 v{phase.update.version}
          </span>
          <Button size="sm" onClick={() => void install(phase.update)}>
            更新并重启
          </Button>
        </>
      )}
      {phase.name === "installing" && (
        <span className="text-muted-foreground">
          正在下载更新
          {typeof phase.progress === "number" ? ` ${Math.round(phase.progress * 100)}%` : "…"}
        </span>
      )}
      {phase.name === "upToDate" && <span className="text-muted-foreground">已是最新版本</span>}
      {phase.name === "error" && <span className="text-destructive">{phase.message}</span>}
      {phase.name !== "available" && phase.name !== "installing" && (
        <Button
          size="sm"
          variant="ghost"
          disabled={phase.name === "checking"}
          onClick={() => void checkManually()}
        >
          检查更新
        </Button>
      )}
    </div>
  );
}
