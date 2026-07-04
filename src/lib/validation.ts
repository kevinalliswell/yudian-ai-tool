import { z } from "zod";

import type { ValidationLimits } from "@/lib/types";

export function createSlaveAddressSchema(limits: ValidationLimits) {
  return z.number().int().min(limits.slaveAddrMin).max(limits.slaveAddrMax);
}

export function createTemperatureSchema(limits: ValidationLimits) {
  return z.number().min(limits.tempMin).max(limits.tempMax);
}

export function createPidSchema(limits: ValidationLimits) {
  return z.object({
    p: z.number().min(0).max(limits.pidPMax),
    i: z.number().int().min(0).max(limits.pidIMax),
    d: z.number().min(0).max(limits.pidDMax),
  });
}

export function createCurveSchema(limits: ValidationLimits) {
  return z
    .array(
      z.object({
        temperature: createTemperatureSchema(limits),
        minutes: z.number().int().min(0),
      }),
    )
    .min(1)
    .max(limits.segmentMaxCount);
}
