import { z } from "zod";

import type { ValidationLimits } from "@/lib/types";

const DEFAULT_SEGMENT_MINUTES_MAX = 65535;

export function createSlaveAddressSchema(limits: ValidationLimits) {
  return z.number().int().min(limits.slaveAddrMin).max(limits.slaveAddrMax);
}

export function createTemperatureSchema(limits: ValidationLimits) {
  return z.number().finite().min(limits.tempMin).max(limits.tempMax);
}

export function createPidSchema(limits: ValidationLimits) {
  return z.object({
    p: z.number().finite().min(0).max(limits.pidPMax),
    i: z.number().finite().int().min(0).max(limits.pidIMax),
    d: z.number().finite().min(0).max(limits.pidDMax),
  });
}

export function createCurveSchema(limits: ValidationLimits) {
  const segmentMinutesMax = limits.segmentMinutesMax ?? DEFAULT_SEGMENT_MINUTES_MAX;
  return z
    .array(
      z.object({
        temperature: createTemperatureSchema(limits),
        minutes: z.number().finite().int().min(0).max(segmentMinutesMax),
      }),
    )
    .min(1)
    .max(limits.segmentMaxCount);
}
