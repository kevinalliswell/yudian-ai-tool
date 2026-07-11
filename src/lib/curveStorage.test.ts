import { afterEach, describe, expect, it } from "vitest";

import snapshot from "@/mocks/snapshots/normal.json";
import type { CurvePreset } from "@/lib/types";

import { loadCurvePresets, saveCurvePresets } from "./curveStorage";

const validPreset = {
  id: "preset-1",
  name: "升温",
  description: "1 段，20 分钟",
  segments: [{ temperature: 100, minutes: 20 }],
};

describe("curve storage", () => {
  afterEach(() => {
    localStorage.clear();
  });

  it("drops malformed presets while retaining valid entries", async () => {
    localStorage.setItem(
      "curvePresets",
      JSON.stringify([
        validPreset,
        { id: "missing-segments", name: "损坏", description: "", segments: null },
      ]),
    );

    await expect(loadCurvePresets(snapshot.validationLimits)).resolves.toEqual([validPreset]);
  });

  it("does not persist malformed presets", async () => {
    const presets = [
      validPreset,
      { id: "missing-segments", name: "损坏", description: "", segments: null },
    ] as unknown as CurvePreset[];

    await saveCurvePresets(presets, snapshot.validationLimits);

    expect(JSON.parse(localStorage.getItem("curvePresets") ?? "null")).toEqual([validPreset]);
  });
});
