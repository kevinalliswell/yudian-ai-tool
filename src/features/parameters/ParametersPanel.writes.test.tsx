import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";

import { api } from "@/lib/api";
import { loadAuditEntries } from "@/lib/auditLog";
import snapshot from "@/mocks/snapshots/normal.json";
import { useDeviceStore } from "@/stores/deviceStore";

import { ParametersPanel } from "./ParametersPanel";

// Uses the frontend mock device behind `api`; failures are injected with spies.
async function connectAndSync() {
  const store = useDeviceStore.getState();
  store.setLimits(snapshot.validationLimits);
  store.applyDeviceStatus(await api.connect({ port: "COM_MOCK", slaveAddr: 1, baudrate: 9600 }));
  store.setSetpoint(await api.readSetpoint());
  store.setPid(await api.readPid());
  store.setParameterSync("synced");
}

function setpointInput() {
  return screen.getAllByRole("spinbutton")[0];
}

describe("ParametersPanel writes", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  afterEach(async () => {
    await api.disconnect().catch(() => undefined);
    useDeviceStore.setState(useDeviceStore.getInitialState(), true);
    vi.restoreAllMocks();
  });

  it("writes a valid setpoint and audits before/after", async () => {
    await connectAndSync();
    // The mock device keeps state across tests, so read the baseline.
    const before = useDeviceStore.getState().setpoint;
    const target = before + 25;
    const write = vi.spyOn(api, "writeSetpoint");
    render(<ParametersPanel />);

    fireEvent.change(setpointInput(), { target: { value: String(target) } });
    fireEvent.click(screen.getByRole("button", { name: "设置" }));

    await waitFor(() => expect(write).toHaveBeenCalledWith(target));
    expect(useDeviceStore.getState().setpoint).toBe(target);
    await waitFor(async () =>
      expect(await loadAuditEntries()).toContainEqual(
        expect.objectContaining({
          action: "write_setpoint",
          outcome: "success",
          details: expect.objectContaining({ before, after: target }),
        }),
      ),
    );
  });

  it("rejects an out-of-range setpoint before it reaches the device", async () => {
    await connectAndSync();
    const write = vi.spyOn(api, "writeSetpoint");
    render(<ParametersPanel />);

    fireEvent.change(setpointInput(), { target: { value: "5000" } });
    fireEvent.click(screen.getByRole("button", { name: "设置" }));

    await waitFor(() => expect(useDeviceStore.getState().error).toBe("给定值超出范围"));
    expect(write).not.toHaveBeenCalled();
    expect(await loadAuditEntries()).toContainEqual(
      expect.objectContaining({
        action: "write_setpoint",
        outcome: "rejected",
        details: expect.objectContaining({ reason: "value_out_of_range" }),
      }),
    );
  });

  it("keeps the previous setpoint when the device rejects the write", async () => {
    await connectAndSync();
    const before = useDeviceStore.getState().setpoint;
    vi.spyOn(api, "writeSetpoint").mockRejectedValueOnce("写入超时");
    render(<ParametersPanel />);

    fireEvent.change(setpointInput(), { target: { value: String(before + 25) } });
    fireEvent.click(screen.getByRole("button", { name: "设置" }));

    await waitFor(() => expect(useDeviceStore.getState().error).toBe("写入超时"));
    expect(useDeviceStore.getState().setpoint).toBe(before);
    expect(await loadAuditEntries()).toContainEqual(
      expect.objectContaining({ action: "write_setpoint", outcome: "failure" }),
    );
  });

  it("writes PID values and stores them as the synced values", async () => {
    await connectAndSync();
    const write = vi.spyOn(api, "writePid");
    render(<ParametersPanel />);
    const [, p, i, d] = screen.getAllByRole("spinbutton");

    fireEvent.change(p, { target: { value: "20" } });
    fireEvent.change(i, { target: { value: "240" } });
    fireEvent.change(d, { target: { value: "3" } });
    fireEvent.click(screen.getByRole("button", { name: "写入 PID" }));

    await waitFor(() => expect(write).toHaveBeenCalledWith({ p: 20, i: 240, d: 3 }));
    expect(useDeviceStore.getState().pid).toEqual({ p: 20, i: 240, d: 3 });
  });

  it("disables writes on a read-only device", async () => {
    await connectAndSync();
    useDeviceStore.getState().applyDeviceStatus({
      ...useDeviceStore.getState().deviceInfo,
      writeEnabled: false,
    });
    render(<ParametersPanel />);

    expect(screen.getByRole("button", { name: "设置" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "写入 PID" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "运行" })).toBeDisabled();
  });
});
