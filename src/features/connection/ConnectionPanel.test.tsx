import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";

import { api } from "@/lib/api";
import { loadAuditEntries } from "@/lib/auditLog";
import snapshot from "@/mocks/snapshots/normal.json";
import { useDeviceStore } from "@/stores/deviceStore";

import { ConnectionPanel } from "./ConnectionPanel";

// In tests `api` is the frontend mock, so these exercise the real panel flow.
function prepareStore() {
  const store = useDeviceStore.getState();
  store.setLimits(snapshot.validationLimits);
  store.setPorts(snapshot.ports);
}

describe("ConnectionPanel", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  afterEach(async () => {
    await api.disconnect().catch(() => undefined);
    useDeviceStore.setState(useDeviceStore.getInitialState(), true);
    vi.restoreAllMocks();
  });

  it("keeps connect disabled until a serial port is selected", () => {
    render(<ConnectionPanel />);

    expect(screen.getByRole("button", { name: "连接" })).toBeDisabled();
    expect(screen.getByText("设备状态")).toBeInTheDocument();
  });

  it("connects, syncs device parameters, starts monitoring and audits it", async () => {
    prepareStore();
    const startMonitoring = vi.spyOn(api, "startMonitoring");
    render(<ConnectionPanel />);

    fireEvent.click(screen.getByRole("button", { name: "连接" }));

    await waitFor(() => expect(useDeviceStore.getState().parameterSync).toBe("synced"));
    const state = useDeviceStore.getState();
    expect(state.deviceInfo.connected).toBe(true);
    expect(state.pid).toEqual(snapshot.pid);
    expect(startMonitoring).toHaveBeenCalledWith(1000);
    await waitFor(async () =>
      expect(await loadAuditEntries()).toContainEqual(
        expect.objectContaining({
          action: "connect",
          outcome: "success",
          details: expect.objectContaining({ status: "parameter_sync_synced" }),
        }),
      ),
    );
  });

  it("stays connected but marks parameters unsynced when the sync read fails", async () => {
    prepareStore();
    vi.spyOn(api, "readPid").mockRejectedValueOnce("PID 读取超时");
    render(<ConnectionPanel />);

    fireEvent.click(screen.getByRole("button", { name: "连接" }));

    await waitFor(() => expect(useDeviceStore.getState().parameterSync).toBe("failed"));
    expect(useDeviceStore.getState().deviceInfo.connected).toBe(true);
    expect(useDeviceStore.getState().error).toBe("参数同步失败：PID 读取超时");
  });

  it("rejects an out-of-range slave address without touching the bus", async () => {
    prepareStore();
    useDeviceStore.getState().setConnectionConfig({ slaveAddr: 0 });
    const connect = vi.spyOn(api, "connect");
    render(<ConnectionPanel />);

    fireEvent.click(screen.getByRole("button", { name: "连接" }));

    await waitFor(() => expect(useDeviceStore.getState().error).toBe("从站地址超出范围"));
    expect(connect).not.toHaveBeenCalled();
    expect(await loadAuditEntries()).toContainEqual(
      expect.objectContaining({ action: "connect", outcome: "rejected" }),
    );
  });

  it("disconnects and clears live device data", async () => {
    prepareStore();
    render(<ConnectionPanel />);
    fireEvent.click(screen.getByRole("button", { name: "连接" }));
    await waitFor(() => expect(useDeviceStore.getState().parameterSync).toBe("synced"));

    fireEvent.click(screen.getByRole("button", { name: "断开" }));

    await waitFor(() => expect(useDeviceStore.getState().deviceInfo.connected).toBe(false));
    expect(useDeviceStore.getState().parameterSync).toBe("unknown");
    expect(screen.getByRole("button", { name: "连接" })).toBeEnabled();
  });
});
