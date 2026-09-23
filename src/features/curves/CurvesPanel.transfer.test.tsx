import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";

import { api } from "@/lib/api";
import { loadAuditEntries } from "@/lib/auditLog";
import snapshot from "@/mocks/snapshots/normal.json";
import { useDeviceStore } from "@/stores/deviceStore";

import { CurvesPanel } from "./CurvesPanel";

async function connectDevice() {
  const store = useDeviceStore.getState();
  store.setLimits(snapshot.validationLimits);
  store.applyDeviceStatus(await api.connect({ port: "COM_MOCK", slaveAddr: 1, baudrate: 9600 }));
}

describe("CurvesPanel device transfer", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  afterEach(async () => {
    await api.disconnect().catch(() => undefined);
    useDeviceStore.setState(useDeviceStore.getInitialState(), true);
    vi.restoreAllMocks();
  });

  it("downloads the edited curve and remembers it as verified", async () => {
    await connectDevice();
    const curve = [
      { temperature: 120, minutes: 10 },
      { temperature: 80, minutes: 5 },
    ];
    useDeviceStore.getState().setCurve(curve);
    const download = vi.spyOn(api, "downloadCurve");
    render(<CurvesPanel />);

    fireEvent.click(screen.getByRole("button", { name: "下载" }));

    await waitFor(() => expect(useDeviceStore.getState().verifiedCurve).toEqual(curve));
    expect(download).toHaveBeenCalledWith(curve);
    await waitFor(async () =>
      expect(await loadAuditEntries()).toContainEqual(
        expect.objectContaining({
          action: "curve_download",
          outcome: "success",
          details: expect.objectContaining({ after: curve }),
        }),
      ),
    );
  });

  it("drops the verified curve when a download fails", async () => {
    await connectDevice();
    useDeviceStore.getState().setVerifiedCurve([{ temperature: 100, minutes: 20 }]);
    vi.spyOn(api, "downloadCurve").mockRejectedValueOnce("下载失败");
    render(<CurvesPanel />);

    fireEvent.click(screen.getByRole("button", { name: "下载" }));

    await waitFor(() => expect(useDeviceStore.getState().error).toBe("下载失败"));
    expect(useDeviceStore.getState().verifiedCurve).toBeUndefined();
  });

  it("refuses to download an out-of-range curve", async () => {
    await connectDevice();
    useDeviceStore.getState().setCurve([{ temperature: 5000, minutes: 10 }]);
    const download = vi.spyOn(api, "downloadCurve");
    render(<CurvesPanel />);

    fireEvent.click(screen.getByRole("button", { name: "下载" }));

    await waitFor(() => expect(useDeviceStore.getState().error).toBe("曲线段超出范围"));
    expect(download).not.toHaveBeenCalled();
  });

  it("uploads the device curve into the editor", async () => {
    await connectDevice();
    const deviceCurve = await api.uploadCurve();
    useDeviceStore.getState().setCurve([{ temperature: 1, minutes: 1 }]);
    render(<CurvesPanel />);

    fireEvent.click(screen.getByRole("button", { name: "上传" }));

    await waitFor(() => expect(useDeviceStore.getState().curve).toEqual(deviceCurve));
  });
});
