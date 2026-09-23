import { useMemo, useState } from "react";
import { Download, Plus, Save, Trash2, Upload } from "lucide-react";
import { useShallow } from "zustand/react/shallow";

import { Button } from "@/components/ui/button";
import { api } from "@/lib/api";
import { recordAuditEvent, rollbackStatusFromError } from "@/lib/auditLog";
import type { CurvePreset, Segment } from "@/lib/types";
import { createCurveSchema } from "@/lib/validation";
import { useDeviceStore } from "@/stores/deviceStore";

import { readableError } from "@/features/shared/display";

export function CurvesPanel() {
  const store = useDeviceStore(
    useShallow((state) => ({
      limits: state.limits,
      deviceInfo: state.deviceInfo,
      connectionConfig: state.connectionConfig,
      curve: state.curve,
      presets: state.presets,
      setCurve: state.setCurve,
      setVerifiedCurve: state.setVerifiedCurve,
      setPresets: state.setPresets,
      setError: state.setError,
    })),
  );
  const [presetName, setPresetName] = useState("");
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
    const before = store.curve.map((segment) => ({ ...segment }));
    try {
      const after = await api.uploadCurve();
      store.setCurve(after);
      store.setError(undefined);
      await recordAuditEvent({
        action: "curve_upload",
        outcome: "success",
        details: {
          connection: store.connectionConfig,
          device: store.deviceInfo,
          before,
          after,
        },
      });
    } catch (error) {
      const message = readableError(error);
      store.setError(message);
      await recordAuditEvent({
        action: "curve_upload",
        outcome: "failure",
        details: {
          connection: store.connectionConfig,
          device: store.deviceInfo,
          before,
          error: message,
        },
      });
    }
  }

  async function downloadCurve() {
    if (!store.limits) return;
    const after = store.curve.map((segment) => ({ ...segment }));
    if (!createCurveSchema(store.limits).safeParse(store.curve).success) {
      store.setError("曲线段超出范围");
      await recordAuditEvent({
        action: "curve_download",
        outcome: "rejected",
        details: {
          connection: store.connectionConfig,
          device: store.deviceInfo,
          attempted: after,
          reason: "curve_out_of_range",
        },
      });
      return;
    }
    let before: Segment[] | undefined;
    let beforeReadError: string | undefined;
    try {
      before = await api.uploadCurve();
    } catch (error) {
      beforeReadError = readableError(error);
    }
    try {
      await api.downloadCurve(after);
      store.setVerifiedCurve(after);
      store.setError(undefined);
      await recordAuditEvent({
        action: "curve_download",
        outcome: "success",
        details: {
          connection: store.connectionConfig,
          device: store.deviceInfo,
          before,
          after,
          rollback: "not_applicable",
          ...(beforeReadError ? { reason: `before_read_failed: ${beforeReadError}` } : {}),
        },
      });
    } catch (error) {
      // The device drops its verification as soon as a download starts.
      store.setVerifiedCurve(undefined);
      const message = readableError(error);
      store.setError(message);
      await recordAuditEvent({
        action: "curve_download",
        outcome: "failure",
        details: {
          connection: store.connectionConfig,
          device: store.deviceInfo,
          before,
          attempted: after,
          error: message,
          rollback: rollbackStatusFromError(error),
          ...(beforeReadError ? { reason: `before_read_failed: ${beforeReadError}` } : {}),
        },
      });
    }
  }

  function savePreset() {
    // Invalid presets are dropped on persist, so reject them visibly here.
    if (!store.limits || !createCurveSchema(store.limits).safeParse(store.curve).success) {
      store.setError("曲线段超出范围，无法保存预设");
      return;
    }
    const preset: CurvePreset = {
      id: crypto.randomUUID(),
      name: presetName.trim() || `曲线 ${store.presets.length + 1}`,
      description: `${store.curve.length} 段，${totalMinutes} 分钟`,
      segments: store.curve.map((segment) => ({ ...segment })),
    };
    store.setPresets([preset, ...store.presets]);
    store.setError(undefined);
    setPresetName("");
    void recordAuditEvent({
      action: "preset_save",
      outcome: "success",
      details: {
        connection: store.connectionConfig,
        device: store.deviceInfo,
        before: store.curve,
        after: preset,
      },
    });
  }

  function deletePreset(id: string) {
    store.setPresets(store.presets.filter((preset) => preset.id !== id));
  }

  return (
    <div className="grid gap-5 xl:grid-cols-[280px_1fr]">
      <div className="rounded-md border bg-background p-4">
        <p className="mb-3 font-medium">曲线预设</p>
        <div className="mb-3 flex gap-2">
          <input
            className="h-10 min-w-0 flex-1 rounded-md border bg-background px-3 text-sm"
            aria-label="预设名称"
            placeholder={`曲线 ${store.presets.length + 1}`}
            value={presetName}
            onChange={(event) => setPresetName(event.target.value)}
          />
          <Button size="icon" variant="outline" aria-label="保存为预设" onClick={savePreset}>
            <Save className="h-4 w-4" aria-hidden="true" />
          </Button>
        </div>
        <div className="grid gap-2">
          {store.presets.map((preset) => (
            <div key={preset.id} className="flex items-stretch gap-2">
              <button
                className="min-w-0 flex-1 rounded-md border p-3 text-left text-sm hover:bg-muted"
                onClick={() => store.setCurve(preset.segments.map((segment) => ({ ...segment })))}
              >
                <span className="block font-medium">{preset.name}</span>
                <span className="text-muted-foreground">{preset.description}</span>
              </button>
              <Button
                variant="outline"
                size="icon"
                className="h-auto"
                aria-label={`删除预设 ${preset.name}`}
                onClick={() => deletePreset(preset.id)}
              >
                <Trash2 className="h-4 w-4" aria-hidden="true" />
              </Button>
            </div>
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
            <Button
              onClick={downloadCurve}
              disabled={!store.deviceInfo.connected || !store.deviceInfo.writeEnabled}
            >
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
                aria-label={`第 ${index + 1} 段温度 (℃)`}
                value={segment.temperature}
                onChange={(event) =>
                  updateSegment(index, { temperature: Number(event.target.value) })
                }
              />
              <input
                className="h-10 rounded-md border bg-background px-3"
                type="number"
                aria-label={`第 ${index + 1} 段时间 (分钟)`}
                value={segment.minutes}
                onChange={(event) => updateSegment(index, { minutes: Number(event.target.value) })}
              />
              <Button
                variant="outline"
                size="icon"
                aria-label={`删除第 ${index + 1} 段`}
                onClick={() => removeSegment(index)}
              >
                <Trash2 className="h-4 w-4" aria-hidden="true" />
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
