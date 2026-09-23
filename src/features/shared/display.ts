/** `decimals` should follow the controller's dPt for values in PV units. */
export function formatValue(value: number | null | undefined, suffix: string, decimals = 1) {
  return typeof value === "number" ? `${value.toFixed(decimals)}${suffix}` : "--";
}

export { describeError as readableError } from "@/i18n/errors";
