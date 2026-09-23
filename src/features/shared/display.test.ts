import { describe, expect, it } from "vitest";

import { formatValue } from "./display";

describe("formatValue", () => {
  it("shows the precision the controller reports", () => {
    expect(formatValue(12.34, " ℃", 2)).toBe("12.34 ℃");
    expect(formatValue(100, " ℃", 0)).toBe("100 ℃");
  });

  it("defaults to one decimal and shows a placeholder without data", () => {
    expect(formatValue(50, " %")).toBe("50.0 %");
    expect(formatValue(null, " ℃", 2)).toBe("--");
    expect(formatValue(undefined, " ℃")).toBe("--");
  });
});
