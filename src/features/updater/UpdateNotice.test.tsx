import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";

import { api } from "@/lib/api";
import { confirmAction } from "@/lib/dialog";
import { checkForUpdate, updatesSupported, type AvailableUpdate } from "@/lib/updater";
import { useDeviceStore } from "@/stores/deviceStore";

import { UpdateNotice } from "./UpdateNotice";

vi.mock("@/lib/api", () => ({
  api: { stopMonitoring: vi.fn(), disconnect: vi.fn() },
}));
vi.mock("@/lib/dialog", () => ({ confirmAction: vi.fn() }));
vi.mock("@/lib/updater", () => ({
  updatesSupported: vi.fn(),
  checkForUpdate: vi.fn(),
}));

const check = vi.mocked(checkForUpdate);
const confirm = vi.mocked(confirmAction);

function update(install = vi.fn().mockResolvedValue(undefined)): AvailableUpdate {
  return { version: "0.7.0", currentVersion: "0.6.0", notes: "修复若干问题", install };
}

describe("UpdateNotice", () => {
  beforeEach(() => {
    vi.mocked(updatesSupported).mockReturnValue(true);
    vi.mocked(api.stopMonitoring).mockResolvedValue(undefined);
    vi.mocked(api.disconnect).mockResolvedValue(undefined);
  });

  afterEach(() => {
    useDeviceStore.setState(useDeviceStore.getInitialState(), true);
    vi.clearAllMocks();
  });

  it("renders nothing outside the desktop runtime", () => {
    vi.mocked(updatesSupported).mockReturnValue(false);
    const { container } = render(<UpdateNotice />);

    expect(container).toBeEmptyDOMElement();
    expect(check).not.toHaveBeenCalled();
  });

  it("announces an available update found by the startup check", async () => {
    check.mockResolvedValue(update());
    render(<UpdateNotice />);

    expect(await screen.findByText("发现新版本 v0.7.0")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "更新并重启" })).toBeInTheDocument();
  });

  it("stays quiet when the startup check fails", async () => {
    check.mockRejectedValue(new Error("offline"));
    render(<UpdateNotice />);

    await waitFor(() => expect(check).toHaveBeenCalled());
    expect(screen.queryByText(/offline/)).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "检查更新" })).toBeInTheDocument();
  });

  it("reports the result of a manual check", async () => {
    check.mockResolvedValue(null);
    render(<UpdateNotice />);
    await waitFor(() => expect(check).toHaveBeenCalledTimes(1));

    fireEvent.click(screen.getByRole("button", { name: "检查更新" }));

    expect(await screen.findByText("已是最新版本")).toBeInTheDocument();
  });

  it("disconnects the device before installing and relaunching", async () => {
    const install = vi.fn().mockResolvedValue(undefined);
    check.mockResolvedValue(update(install));
    confirm.mockResolvedValue(true);
    useDeviceStore.getState().applyDeviceStatus({
      connected: true,
      writeEnabled: true,
      decimalPoint: 1,
      scaleFactor: 1,
    });
    render(<UpdateNotice />);

    fireEvent.click(await screen.findByRole("button", { name: "更新并重启" }));

    await waitFor(() => expect(install).toHaveBeenCalled());
    expect(confirm.mock.calls[0][0]).toContain("设备连接会先断开");
    const disconnectOrder = vi.mocked(api.disconnect).mock.invocationCallOrder[0];
    expect(disconnectOrder).toBeLessThan(install.mock.invocationCallOrder[0]);
  });

  it("does not install when the confirmation is cancelled", async () => {
    const install = vi.fn();
    check.mockResolvedValue(update(install));
    confirm.mockResolvedValue(false);
    render(<UpdateNotice />);

    fireEvent.click(await screen.findByRole("button", { name: "更新并重启" }));

    await waitFor(() => expect(confirm).toHaveBeenCalled());
    expect(install).not.toHaveBeenCalled();
    expect(api.disconnect).not.toHaveBeenCalled();
  });
});
