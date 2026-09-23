import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { fireEvent, render, screen, within } from "@testing-library/react";

import snapshot from "@/mocks/snapshots/normal.json";
import { useDeviceStore } from "@/stores/deviceStore";

import { CurvesPanel } from "./CurvesPanel";

describe("CurvesPanel", () => {
  beforeEach(() => {
    useDeviceStore.getState().setLimits(snapshot.validationLimits);
  });

  afterEach(() => {
    useDeviceStore.setState(useDeviceStore.getInitialState(), true);
  });

  it("adds a curve segment with labelled inputs", () => {
    render(<CurvesPanel />);
    expect(screen.getAllByRole("spinbutton")).toHaveLength(2);

    fireEvent.click(screen.getByRole("button", { name: "增加段" }));

    expect(screen.getByLabelText("第 2 段温度 (℃)")).toBeInTheDocument();
    expect(screen.getByLabelText("第 2 段时间 (分钟)")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "删除第 2 段" })).toBeInTheDocument();
  });

  it("saves a named preset and deletes it again", () => {
    render(<CurvesPanel />);

    fireEvent.change(screen.getByLabelText("预设名称"), { target: { value: "退火曲线" } });
    fireEvent.click(screen.getByRole("button", { name: "保存为预设" }));

    const presets = useDeviceStore.getState().presets;
    expect(presets).toHaveLength(1);
    expect(presets[0].name).toBe("退火曲线");

    fireEvent.click(screen.getByRole("button", { name: "删除预设 退火曲线" }));
    expect(useDeviceStore.getState().presets).toHaveLength(0);
  });

  it("refuses to save an out-of-range curve instead of dropping it later", () => {
    useDeviceStore.getState().setCurve([{ temperature: 5000, minutes: 10 }]);
    render(<CurvesPanel />);

    fireEvent.click(screen.getByRole("button", { name: "保存为预设" }));

    expect(useDeviceStore.getState().presets).toHaveLength(0);
    expect(useDeviceStore.getState().error).toBe("曲线段超出范围，无法保存预设");
  });

  it("loads a preset into the editor", () => {
    useDeviceStore.getState().setPresets([
      {
        id: "p1",
        name: "保温",
        description: "1 段，30 分钟",
        segments: [{ temperature: 150, minutes: 30 }],
      },
    ]);
    render(<CurvesPanel />);

    fireEvent.click(screen.getByRole("button", { name: /^保温/ }));

    const row = screen.getByLabelText("第 1 段温度 (℃)").closest("div") as HTMLElement;
    expect(within(row).getByLabelText("第 1 段温度 (℃)")).toHaveValue(150);
  });
});
