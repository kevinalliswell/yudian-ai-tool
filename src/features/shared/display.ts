export function formatValue(value: number | null | undefined, suffix: string) {
  return typeof value === "number" ? `${value.toFixed(1)}${suffix}` : "--";
}

export function readableError(error: unknown) {
  if (typeof error === "string") return error;
  if (error && typeof error === "object") {
    const record = error as { message?: unknown; kind?: unknown };
    if (record.message != null && record.message !== "") return String(record.message);
    if (typeof record.kind === "string") return record.kind;
  }
  return String(error);
}
