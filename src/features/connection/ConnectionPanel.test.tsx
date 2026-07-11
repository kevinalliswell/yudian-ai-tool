import { afterEach, describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";

import { ConnectionPanel } from "./ConnectionPanel";
import { useDeviceStore } from "@/stores/deviceStore";

describe("ConnectionPanel", () => {
  afterEach(() => {
    useDeviceStore.setState(useDeviceStore.getInitialState(), true);
  });

  it("keeps connect disabled until a serial port is selected", () => {
    render(<ConnectionPanel />);

    expect(screen.getByRole("button", { name: "连接" })).toBeDisabled();
    expect(screen.getByText("设备状态")).toBeInTheDocument();
  });
});
