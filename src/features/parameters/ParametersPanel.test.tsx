import { afterEach, describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";

import { useDeviceStore } from "@/stores/deviceStore";

import { ParametersPanel } from "./ParametersPanel";

describe("ParametersPanel", () => {
  afterEach(() => {
    useDeviceStore.setState(useDeviceStore.getInitialState(), true);
  });

  it("disables device parameter writes while disconnected", () => {
    render(<ParametersPanel />);

    expect(screen.getByRole("button", { name: "读取当前值" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "设置" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "写入 PID" })).toBeDisabled();
  });
});
