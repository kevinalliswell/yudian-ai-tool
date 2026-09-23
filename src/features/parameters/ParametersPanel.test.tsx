import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";

import { api } from "@/lib/api";
import { confirmAction } from "@/lib/dialog";
import { zhCN } from "@/i18n/zh-CN";
import { useDeviceStore } from "@/stores/deviceStore";

import { ParametersPanel } from "./ParametersPanel";

vi.mock("@/lib/api", () => ({
  api: { setRunStatus: vi.fn() },
}));
vi.mock("@/lib/dialog", () => ({
  confirmAction: vi.fn(),
}));

const setRunStatus = vi.mocked(api.setRunStatus);
const confirm = vi.mocked(confirmAction);

function connectWritableDevice() {
  useDeviceStore.getState().applyDeviceStatus({
    connected: true,
    writeEnabled: true,
    modelCode: 5167,
    modelName: "AI-516P",
    decimalPoint: 1,
    scaleFactor: 1,
  });
}

describe("ParametersPanel", () => {
  beforeEach(() => {
    setRunStatus.mockResolvedValue(undefined);
  });

  afterEach(() => {
    useDeviceStore.setState(useDeviceStore.getInitialState(), true);
    vi.clearAllMocks();
  });

  it("disables device parameter writes while disconnected", () => {
    render(<ParametersPanel />);

    expect(screen.getByRole("button", { name: "读取当前值" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "设置" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "写入 PID" })).toBeDisabled();
  });

  it("blocks run until a curve has been downloaded and verified", async () => {
    connectWritableDevice();
    render(<ParametersPanel />);

    fireEvent.click(screen.getByRole("button", { name: "运行" }));

    await waitFor(() =>
      expect(useDeviceStore.getState().error).toBe(zhCN.runConfirmation.notVerified),
    );
    expect(confirm).not.toHaveBeenCalled();
    expect(setRunStatus).not.toHaveBeenCalled();
  });

  it("confirms the verified device curve and warns when the editor differs", async () => {
    connectWritableDevice();
    useDeviceStore.getState().setVerifiedCurve([{ temperature: 120, minutes: 10 }]);
    useDeviceStore.getState().setCurve([{ temperature: 100, minutes: 20 }]);
    confirm.mockResolvedValue(true);
    render(<ParametersPanel />);

    fireEvent.click(screen.getByRole("button", { name: "运行" }));

    await waitFor(() => expect(setRunStatus).toHaveBeenCalledWith("run"));
    const [message, title] = confirm.mock.calls[0];
    expect(title).toBe(zhCN.runConfirmation.title);
    expect(message).toContain(zhCN.runConfirmation.summary(1, 10));
    expect(message).toContain(zhCN.runConfirmation.mismatch);
  });

  it("does not run when the confirmation is cancelled", async () => {
    connectWritableDevice();
    const curve = [{ temperature: 120, minutes: 10 }];
    useDeviceStore.getState().setVerifiedCurve(curve);
    useDeviceStore.getState().setCurve(curve);
    confirm.mockResolvedValue(false);
    render(<ParametersPanel />);

    fireEvent.click(screen.getByRole("button", { name: "运行" }));

    await waitFor(() => expect(confirm).toHaveBeenCalled());
    expect(confirm.mock.calls[0][0]).not.toContain(zhCN.runConfirmation.mismatch);
    expect(setRunStatus).not.toHaveBeenCalled();
  });
});
