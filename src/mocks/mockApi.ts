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
  UnlistenFn,
  ValidationLimits,
} from "@/lib/types";

let connected = false;
let pid: PidValues = { ...snapshot.pid };
let setpoint = 100;
let curve: Segment[] = snapshot.segments.map((segment) => ({ ...segment }));
let curveVerified = false;
let runStatus: RunStatus = "stop";
let timer: ReturnType<typeof setInterval> | undefined;
let streamIndex = 0;

const readingListeners = new Set<(payload: Reading) => void>();
const statusListeners = new Set<(payload: DeviceInfo) => void>();
const errorListeners = new Set<(payload: ErrorEvent) => void>();

function emitReading(payload: Reading) {
  for (const listener of readingListeners) listener(payload);
}

function emitStatus(payload: DeviceInfo) {
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
    runStatus = "stop";
    const info = { ...snapshot.deviceInfo, connected };
    emitStatus(info);
    return info;
  },

  async disconnect(): Promise<void> {
    connected = false;
    curveVerified = false;
    await mockApi.stopMonitoring();
    emitStatus({ connected: false, writeEnabled: false, decimalPoint: 1, scaleFactor: 1 });
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
    if (status !== "run") {
      runStatus = status;
      return;
    }
    if (!snapshot.deviceInfo.modelName) {
      throw { kind: "runBlocked", reason: "unsupportedModel", message: "run blocked" };
    }
    if (!curveVerified) {
      throw { kind: "runBlocked", reason: "curveNotVerified", message: "run blocked" };
    }
    const reading = snapshot.readingStream[0];
    const { tempMin, tempMax } = snapshot.validationLimits;
    for (const [reason, value] of [
      ["invalidPv", reading.pv],
      ["invalidSv", setpoint],
    ] as const) {
      if (typeof value !== "number" || !Number.isFinite(value)) {
        throw { kind: "runBlocked", reason, message: "run blocked" };
      }
      if (value < tempMin || value > tempMax) {
        throw { kind: "runBlocked", reason, message: "run blocked" };
      }
    }
    runStatus = "run";
  },

  async uploadCurve(): Promise<Segment[]> {
    ensureConnected();
    return curve.map((segment) => ({ ...segment }));
  },

  async downloadCurve(segments: Segment[]): Promise<void> {
    ensureConnected();
    if (runStatus === "run") {
      throw { kind: "deviceRunning", message: "程序运行中，请先暂停(HoLd)或停止后再下载曲线" };
    }
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

  onStatus(callback: (payload: DeviceInfo) => void): Promise<UnlistenFn> {
    return subscribe(statusListeners, callback);
  },

  onError(callback: (payload: ErrorEvent) => void): Promise<UnlistenFn> {
    return subscribe(errorListeners, callback);
  },
};
