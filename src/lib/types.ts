export interface PortInfo {
  name: string;
  description?: string;
}

export interface ConnectionConfig {
  port: string;
  slaveAddr: number;
  baudrate: number;
}

export interface DeviceInfo {
  connected: boolean;
  modelCode?: number;
  modelName?: string;
  decimalPoint: number;
  scaleFactor: number;
}

export interface PidValues {
  p: number;
  i: number;
  d: number;
}

export interface Segment {
  temperature: number;
  minutes: number;
}

export interface CurvePreset {
  id: string;
  name: string;
  description: string;
  segments: Segment[];
}

export type RunStatus = "run" | "hold" | "stop";

export interface ValidationLimits {
  tempMin: number;
  tempMax: number;
  pidPMax: number;
  pidIMax: number;
  pidDMax: number;
  segmentMaxCount: number;
  slaveAddrMin: number;
  slaveAddrMax: number;
  refreshIntervalMinMs: number;
}

export interface Reading {
  pv?: number | null;
  sv?: number | null;
  mv?: number | null;
  ts: number;
}

export interface StatusEvent {
  connected: boolean;
  model?: string | null;
}

export interface ErrorEvent {
  scope: string;
  message: string;
}

export interface AppError {
  kind: string;
  message?: string;
  [key: string]: unknown;
}

export type UnlistenFn = () => void;

export interface DeviceApi {
  listSerialPorts: () => Promise<PortInfo[]>;
  connect: (cfg: ConnectionConfig) => Promise<DeviceInfo>;
  disconnect: () => Promise<void>;
  getDeviceInfo: () => Promise<DeviceInfo>;
  getValidationLimits: () => Promise<ValidationLimits>;
  readPid: () => Promise<PidValues>;
  writeSetpoint: (value: number) => Promise<void>;
  writePid: (values: PidValues) => Promise<void>;
  setRunStatus: (status: RunStatus) => Promise<void>;
  uploadCurve: () => Promise<Segment[]>;
  downloadCurve: (segments: Segment[]) => Promise<void>;
  startMonitoring: (intervalMs: number) => Promise<void>;
  stopMonitoring: () => Promise<void>;
  onReading: (callback: (payload: Reading) => void) => Promise<UnlistenFn>;
  onStatus: (callback: (payload: StatusEvent) => void) => Promise<UnlistenFn>;
  onError: (callback: (payload: ErrorEvent) => void) => Promise<UnlistenFn>;
}
