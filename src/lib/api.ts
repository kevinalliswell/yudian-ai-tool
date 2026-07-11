import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

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
  ValidationLimits,
} from "@/lib/types";
import { mockApi } from "@/mocks/mockApi";

const hasTauriRuntime = "__TAURI_INTERNALS__" in window;

export const isFrontendMockEnabled =
  import.meta.env.DEV && (import.meta.env.VITE_MOCK === "true" || !hasTauriRuntime);

const realApi: DeviceApi = {
  listSerialPorts: () => invoke<PortInfo[]>("list_serial_ports"),
  connect: (cfg: ConnectionConfig) => invoke<DeviceInfo>("connect", { cfg }),
  disconnect: () => invoke<void>("disconnect"),
  getDeviceInfo: () => invoke<DeviceInfo>("get_device_info"),
  getValidationLimits: () => invoke<ValidationLimits>("get_validation_limits"),
  readPid: () => invoke<PidValues>("read_pid"),
  readSetpoint: () => invoke<number>("read_setpoint"),
  writeSetpoint: (value: number) => invoke<void>("write_setpoint", { value }),
  writePid: (values: PidValues) => invoke<void>("write_pid", { values }),
  setRunStatus: (status: RunStatus) => invoke<void>("set_run_status", { status }),
  uploadCurve: () => invoke<Segment[]>("upload_curve"),
  downloadCurve: (segments: Segment[]) => invoke<void>("download_curve", { segments }),
  startMonitoring: (intervalMs: number) => invoke<void>("start_monitoring", { intervalMs }),
  stopMonitoring: () => invoke<void>("stop_monitoring"),
  onReading: async (callback: (payload: Reading) => void) =>
    listen<Reading>("device://reading", (event) => callback(event.payload)),
  onStatus: async (callback: (payload: StatusEvent) => void) =>
    listen<StatusEvent>("device://status", (event) => callback(event.payload)),
  onError: async (callback: (payload: ErrorEvent) => void) =>
    listen<ErrorEvent>("device://error", (event) => callback(event.payload)),
};

export const api: DeviceApi = isFrontendMockEnabled ? mockApi : realApi;
export const runtimeLabel = isFrontendMockEnabled ? "Vite Mock" : "Tauri Runtime";
