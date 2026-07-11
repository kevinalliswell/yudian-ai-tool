import { useMemo } from "react";
import { Download, Plus, Save, Trash2, Upload } from "lucide-react";
import { useShallow } from "zustand/react/shallow";

import { Button } from "@/components/ui/button";
import { api } from "@/lib/api";
import type { CurvePreset, Segment } from "@/lib/types";
import { createCurveSchema } from "@/lib/validation";
import { useDeviceStore } from "@/stores/deviceStore";

import { readableError } from "@/features/shared/display";

export function CurvesPanel() {
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
