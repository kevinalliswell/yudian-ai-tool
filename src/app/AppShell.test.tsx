import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";

import { api } from "@/lib/api";
import snapshot from "@/mocks/snapshots/normal.json";
import { useAppStore } from "@/stores/appStore";
import { useDeviceStore } from "@/stores/deviceStore";

import { AppShell } from "./AppShell";

describe("AppShell", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  afterEach(async () => {
    await api.disconnect().catch(() => undefined);
    useDeviceStore.setState(useDeviceStore.getInitialState(), true);
    useAppStore.setState(useAppStore.getInitialState(), true);
  });

  it("loads limits, ports and stored presets on boot", async () => {
    localStorage.setItem(
      "curvePresets",
      JSON.stringify([
        {
          id: "p1",
          name: "保温",
          description: "1 段，30 分钟",
          segments: [{ temperature: 150, minutes: 30 }],
        },
      ]),
    );
    render(<AppShell />);

    await waitFor(() =>
      expect(useDeviceStore.getState().limits).toEqual(snapshot.validationLimits),
    );
    expect(useDeviceStore.getState().ports).toEqual(snapshot.ports);
    expect(useDeviceStore.getState().presets.map((preset) => preset.name)).toEqual(["保温"]);
  });

  it("follows device status events published by the backend", async () => {
    render(<AppShell />);
    await waitFor(() => expect(useDeviceStore.getState().limits).toBeDefined());

    await act(async () => {
      await api.connect({ port: "COM_MOCK", slaveAddr: 1, baudrate: 9600 });
    });
    expect(await screen.findAllByText("AI-516P")).not.toHaveLength(0);

    await act(async () => {
      await api.disconnect();
    });
    await waitFor(() => expect(useDeviceStore.getState().deviceInfo.connected).toBe(false));
  });

  it("switches between the four panels", async () => {
    render(<AppShell />);

    fireEvent.click(screen.getByRole("button", { name: "温控曲线" }));
    expect(await screen.findByText("段编辑")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "实时监控" }));
    expect(await screen.findByText("PV 时序")).toBeInTheDocument();
  });
});
