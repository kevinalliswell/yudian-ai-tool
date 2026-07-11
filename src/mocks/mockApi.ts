import snapshot from "@/mocks/snapshots/normal.json";
import type {
  ConnectionConfig,
  DeviceApi,
  DeviceInfo,
  ErrorEvent,
  PidValues,
  PortInfo,
  Reading,
  RunStatus,
  Segment,
  StatusEvent,
  UnlistenFn,
  ValidationLimits,
} from "@/lib/types";

let connected = false;
let pid: PidValues = { ...snapshot.pid };
let setpoint = 100;
let curve: Segment[] = snapshot.segments.map((segment) => ({ ...segment }));
let curveVerified = false;
let timer: ReturnType<typeof setInterval> | undefined;
let streamIndex = 0;

const readingListeners = new Set<(payload: Reading) => void>();
const statusListeners = new Set<(payload: StatusEvent) => void>();
const errorListeners = new Set<(payload: ErrorEvent) => void>();

function emitReading(payload: Reading) {
  for (const listener of readingListeners) listener(payload);
}

function emitStatus(payload: StatusEvent) {
  for (const listener of statusListeners) listener(payload);
}

function ensureConnected() {
  if (!connected) {
    throw { kind: "notConnected", message: "设备未连接" };
  }
}

function subscribe<T>(
  set: Set<(payload: T) => void>,
  callback: (payload: T) => void,
): Promise<UnlistenFn> {
  set.add(callback);
  return Promise.resolve(() => set.delete(callback));
}

export const mockApi: DeviceApi = {
  async listSerialPorts(): Promise<PortInfo[]> {
    return snapshot.ports;
  },

  async connect(cfg: ConnectionConfig): Promise<DeviceInfo> {
    void cfg;
    connected = true;
    curveVerified = false;
    const info = { ...snapshot.deviceInfo, connected };
    emitStatus({ connected: true, model: info.modelName });
    return info;
  },

  async disconnect(): Promise<void> {
    connected = false;
    curveVerified = false;
    await mockApi.stopMonitoring();
    emitStatus({ connected: false, model: null });
  },

  async getDeviceInfo(): Promise<DeviceInfo> {
    return connected
      ? { ...snapshot.deviceInfo, connected: true }
      : { connected: false, writeEnabled: false, decimalPoint: 1, scaleFactor: 1 };
  },

  async getValidationLimits(): Promise<ValidationLimits> {
    return snapshot.validationLimits;
  },

  async readPid(): Promise<PidValues> {
    ensureConnected();
    return { ...pid };
  },

  async readSetpoint(): Promise<number> {
    ensureConnected();
    return setpoint;
  },

  async writeSetpoint(value: number): Promise<void> {
    ensureConnected();
    setpoint = value;
  },

  async writePid(values: PidValues): Promise<void> {
    ensureConnected();
    pid = { ...values };
  },

  async setRunStatus(status: RunStatus): Promise<void> {
    ensureConnected();
    if (status !== "run") return;
    if (!snapshot.deviceInfo.modelName) {
      throw { kind: "invalidData", message: "运行需要受支持的设备型号" };
    }
    if (!curveVerified) {
      throw { kind: "invalidData", message: "运行需要已验证的曲线下载" };
    }
    const reading = snapshot.readingStream[0];
    const { tempMin, tempMax } = snapshot.validationLimits;
    for (const [label, value] of [
      ["PV", reading.pv],
      ["SV", setpoint],
    ] as const) {
      if (typeof value !== "number" || !Number.isFinite(value)) {
        throw { kind: "invalidData", message: `运行需要有效的 ${label} 数据` };
      }
      if (value < tempMin || value > tempMax) {
        throw { kind: "invalidData", message: `运行需要 ${label} 在温度范围内` };
      }
    }
  },

  async uploadCurve(): Promise<Segment[]> {
    ensureConnected();
    return curve.map((segment) => ({ ...segment }));
  },

  async downloadCurve(segments: Segment[]): Promise<void> {
    ensureConnected();
    curve = segments.map((segment) => ({ ...segment }));
    curveVerified = true;
  },

  async startMonitoring(intervalMs: number): Promise<void> {
    ensureConnected();
    await mockApi.stopMonitoring();
    timer = setInterval(
      () => {
        const item = snapshot.readingStream[streamIndex % snapshot.readingStream.length];
        streamIndex += 1;
        emitReading({
          pv: item.pv,
          sv: setpoint,
          mv: item.mv,
          ts: Date.now(),
        });
      },
      Math.max(intervalMs, snapshot.validationLimits.refreshIntervalMinMs),
    );
  },

  async stopMonitoring(): Promise<void> {
    if (timer) {
      clearInterval(timer);
      timer = undefined;
    }
  },

  onReading(callback: (payload: Reading) => void): Promise<UnlistenFn> {
    return subscribe(readingListeners, callback);
  },

  onStatus(callback: (payload: StatusEvent) => void): Promise<UnlistenFn> {
    return subscribe(statusListeners, callback);
  },

  onError(callback: (payload: ErrorEvent) => void): Promise<UnlistenFn> {
    return subscribe(errorListeners, callback);
  },
};
