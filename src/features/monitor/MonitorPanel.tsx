import { useDeviceStore } from "@/stores/deviceStore";

import { formatValue } from "@/features/shared/display";

export function MonitorPanel() {
  const latestReading = useDeviceStore((state) => state.latestReading);
  const readings = useDeviceStore((state) => state.readings);
  const deviceInfo = useDeviceStore((state) => state.deviceInfo);
  return (
    <div>
      <div className="metric-grid">
        <Metric label="PV 测量值" value={formatValue(latestReading?.pv, " ℃")} />
        <Metric label="SV 给定值" value={formatValue(latestReading?.sv, " ℃")} />
        <Metric label="MV 输出值" value={formatValue(latestReading?.mv, " %")} />
      </div>
      <section className="surface chart-surface">
        <div className="chart-heading">
          <div>
            <p className="surface-kicker">PROCESS TREND</p>
            <h3 className="chart-title">PV 时序</h3>
          </div>
          <span className="chart-count">
            {deviceInfo.connected ? `${readings.length} 点` : "--"}
          </span>
        </div>
        <PvChart readings={readings} />
      </section>
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
      className="pv-chart"
      viewBox="0 0 100 100"
      preserveAspectRatio="none"
      role="img"
      aria-label="PV 温度趋势图"
    >
      <title>PV 温度趋势图</title>
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
    <article className="metric-tile">
      <p className="metric-label">{label}</p>
      <p className="metric-value">{value}</p>
    </article>
  );
}
