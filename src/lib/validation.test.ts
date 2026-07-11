import { describe, expect, it } from "vitest";

import snapshot from "@/mocks/snapshots/normal.json";
import {
  createCurveSchema,
  createPidSchema,
  createSlaveAddressSchema,
  createTemperatureSchema,
} from "./validation";

const limits = snapshot.validationLimits;

describe("validation schemas", () => {
  it("validates temperature boundaries from injected limits", () => {
    const schema = createTemperatureSchema(limits);
    expect(schema.safeParse(-200).success).toBe(true);
    expect(schema.safeParse(1800).success).toBe(true);
    expect(schema.safeParse(-200.1).success).toBe(false);
    expect(schema.safeParse(1800.1).success).toBe(false);
    expect(schema.safeParse(Number.NaN).success).toBe(false);
    expect(schema.safeParse(Number.POSITIVE_INFINITY).success).toBe(false);
  });

  it("validates slave address boundaries from injected limits", () => {
    const schema = createSlaveAddressSchema(limits);
    expect(schema.safeParse(1).success).toBe(true);
    expect(schema.safeParse(80).success).toBe(true);
    expect(schema.safeParse(0).success).toBe(false);
    expect(schema.safeParse(81).success).toBe(false);
  });

  it("validates curve segment count and values", () => {
    const schema = createCurveSchema(limits);
    expect(schema.safeParse([]).success).toBe(false);
    expect(
      schema.safeParse(Array.from({ length: 50 }, () => ({ temperature: 100, minutes: 1 })))
        .success,
    ).toBe(true);
    expect(
      schema.safeParse(Array.from({ length: 51 }, () => ({ temperature: 100, minutes: 1 })))
        .success,
    ).toBe(false);
    expect(schema.safeParse([{ temperature: 100, minutes: -1 }]).success).toBe(false);
    expect(schema.safeParse([{ temperature: Number.NaN, minutes: 1 }]).success).toBe(false);
    expect(schema.safeParse([{ temperature: 100, minutes: 65535 }]).success).toBe(true);
    expect(schema.safeParse([{ temperature: 100, minutes: 65536 }]).success).toBe(false);
  });

  it("rejects non-finite PID values", () => {
    const schema = createPidSchema(limits);
    expect(schema.safeParse({ p: Number.NaN, i: 0, d: 0 }).success).toBe(false);
    expect(schema.safeParse({ p: 0, i: 0, d: Number.POSITIVE_INFINITY }).success).toBe(false);
  });
});
