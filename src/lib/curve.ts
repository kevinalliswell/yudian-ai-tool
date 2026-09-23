import type { Segment } from "@/lib/types";

export function curveTotalMinutes(curve: Segment[]) {
  return curve.reduce((sum, segment) => sum + segment.minutes, 0);
}

export function sameCurve(a: Segment[], b: Segment[]) {
  return (
    a.length === b.length &&
    a.every(
      (segment, index) =>
        segment.temperature === b[index].temperature && segment.minutes === b[index].minutes,
    )
  );
}
