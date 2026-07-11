import { afterEach, describe, expect, it } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";

import { useDeviceStore } from "@/stores/deviceStore";

import { CurvesPanel } from "./CurvesPanel";

describe("CurvesPanel", () => {
  afterEach(() => {
    useDeviceStore.setState(useDeviceStore.getInitialState(), true);
  });

  it("adds a curve segment to the editor", () => {
    render(<CurvesPanel />);
    expect(screen.getAllByRole("spinbutton")).toHaveLength(2);

    fireEvent.click(screen.getByRole("button", { name: "增加段" }));

    expect(screen.getAllByRole("spinbutton")).toHaveLength(4);
  });
});
