import { useMemo } from "react";
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
    const preset: CurvePreset = {
      id: crypto.randomUUID(),
      name: `曲线 ${store.presets.length + 1}`,
      description: `${store.curve.length} 段，${totalMinutes} 分钟`,
      segments: store.curve.map((segment) => ({ ...segment })),
    };
    store.setPresets([preset, ...store.presets]);
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

  return (
    <div className="curves-layout">
      <aside className="surface preset-surface">
        <div className="preset-heading">
          <div>
            <p className="surface-kicker">LIBRARY</p>
            <h3 className="preset-title">曲线预设</h3>
          </div>
          <Button size="icon" variant="outline" onClick={savePreset} aria-label="保存曲线预设">
            <Save className="h-4 w-4" aria-hidden="true" />
          </Button>
        </div>
        <div className="preset-list">
          {store.presets.length ? (
            store.presets.map((preset) => (
              <button
                key={preset.id}
                className="preset-item"
                onClick={() => store.setCurve(preset.segments.map((segment) => ({ ...segment })))}
              >
                <strong>{preset.name}</strong>
                <span>{preset.description}</span>
              </button>
            ))
          ) : (
            <div className="empty-state">暂无已保存曲线</div>
          )}
        </div>
      </aside>

      <section className="surface editor-surface">
        <div className="editor-heading">
          <div>
            <p className="surface-kicker">PROFILE EDITOR</p>
            <h3 className="editor-title">段编辑</h3>
            <p className="editor-summary">
              {store.curve.length} 段 · {totalMinutes} 分钟 · {(totalMinutes / 60).toFixed(1)} 小时
            </p>
          </div>
          <div className="action-row">
            <Button variant="outline" onClick={uploadCurve} disabled={!store.deviceInfo.connected}>
              <Upload className="mr-2 h-4 w-4" aria-hidden="true" />
              上传
            </Button>
            <Button
              onClick={downloadCurve}
              disabled={!store.deviceInfo.connected || !store.deviceInfo.writeEnabled}
            >
              <Download className="mr-2 h-4 w-4" aria-hidden="true" />
              下载
            </Button>
          </div>
        </div>
        <div className="segment-list">
          {store.curve.map((segment, index) => (
            <div key={index} className="segment-row">
              <label className="field">
                温度
                <input
                  className="control-input"
                  aria-label={`第 ${index + 1} 段温度`}
                  type="number"
                  value={segment.temperature}
                  onChange={(event) =>
                    updateSegment(index, { temperature: Number(event.target.value) })
                  }
                />
              </label>
              <label className="field">
                时间（分钟）
                <input
                  className="control-input"
                  aria-label={`第 ${index + 1} 段时间`}
                  type="number"
                  value={segment.minutes}
                  onChange={(event) =>
                    updateSegment(index, { minutes: Number(event.target.value) })
                  }
                />
              </label>
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
          <Plus className="mr-2 h-4 w-4" aria-hidden="true" />
          增加段
        </Button>
      </section>
    </div>
  );
}
