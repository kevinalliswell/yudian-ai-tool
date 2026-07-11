import { Badge } from "@/components/ui/badge";
import { useDeviceStore } from "@/stores/deviceStore";

import { formatValue } from "@/features/shared/display";

export function MonitorPanel() {
  const latestReading = useDeviceStore((state) => state.latestReading);
  const readings = useDeviceStore((state) => state.readings);
  const deviceInfo = useDeviceStore((state) => state.deviceInfo);
  return (
    <div className="grid gap-5">
      <div className="grid gap-3 md:grid-cols-3">
        <Metric label="PV 测量值" value={formatValue(latestReading?.pv, " ℃")} />
        <Metric label="SV 给定值" value={formatValue(latestReading?.sv, " ℃")} />
        <Metric label="MV 输出值" value={formatValue(latestReading?.mv, " %")} />
      </div>
      <div className="rounded-md border bg-background p-4">
        <div className="mb-3 flex items-center justify-between">
          <p className="font-medium">PV 时序</p>
          <Badge variant="outline">{deviceInfo.connected ? `${readings.length} 点` : "--"}</Badge>
        </div>
        <PvChart readings={readings} />
      </div>
    </div>
  );
}

function PvChart({ readings }: { readings: Array<{ pv?: number | null }> }) {
  const points = readings.filter((reading) => typeof reading.pv === "number");
  const values = points.map((reading) => reading.pv as number);
  const min = values.length ? Math.min(...values) : 0;
  const max = values.length ? Math.max(...values) : 1;
  const range = Math.max(max - min, 1);
  const path = values
    .map((value, index) => {
      const x = values.length === 1 ? 0 : (index / (values.length - 1)) * 100;
      const y = 100 - ((value - min) / range) * 100;
      return `${index === 0 ? "M" : "L"} ${x.toFixed(2)} ${y.toFixed(2)}`;
    })
    .join(" ");

  return (
    <svg
      className="h-64 w-full rounded-md border bg-card"
      viewBox="0 0 100 100"
      preserveAspectRatio="none"
    >
      <path
        d={path}
        fill="none"
        stroke="hsl(var(--primary))"
        strokeWidth="1.8"
        vectorEffect="non-scaling-stroke"
      />
    </svg>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-md border bg-background p-4">
      <p className="text-sm text-muted-foreground">{label}</p>
      <p className="mt-2 text-3xl font-semibold tracking-normal">{value}</p>
    </div>
  );
}
