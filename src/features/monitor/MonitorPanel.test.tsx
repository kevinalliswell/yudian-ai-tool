import { afterEach, describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";

import { useDeviceStore } from "@/stores/deviceStore";

import { MonitorPanel } from "./MonitorPanel";

describe("MonitorPanel", () => {
  afterEach(() => {
    useDeviceStore.setState(useDeviceStore.getInitialState(), true);
  });

  it("shows empty readings while disconnected", () => {
    render(<MonitorPanel />);

    expect(screen.getAllByText("--")).toHaveLength(4);
    expect(screen.getByText("PV 时序")).toBeInTheDocument();
  });
});
