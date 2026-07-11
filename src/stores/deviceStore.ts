import { create } from "zustand";

import type {
  ConnectionConfig,
  CurvePreset,
  DeviceInfo,
  ErrorEvent,
  PidValues,
  PortInfo,
  ParameterSyncState,
  Reading,
  Segment,
  ValidationLimits,
} from "@/lib/types";

interface DeviceState {
  ports: PortInfo[];
  connectionConfig: ConnectionConfig;
  deviceInfo: DeviceInfo;
  limits?: ValidationLimits;
  latestReading?: Reading;
  readings: Reading[];
  error?: string;
  pid: PidValues;
  setpoint: number;
  parameterSync: ParameterSyncState;
  curve: Segment[];
  presets: CurvePreset[];
  setPorts: (ports: PortInfo[]) => void;
  setConnectionConfig: (config: Partial<ConnectionConfig>) => void;
  setDeviceInfo: (info: DeviceInfo) => void;
  setLimits: (limits: ValidationLimits) => void;
  pushReading: (reading: Reading) => void;
  setError: (error?: string) => void;
  setBackendError: (event: ErrorEvent) => void;
  setPid: (pid: PidValues) => void;
  setSetpoint: (setpoint: number) => void;
  setParameterSync: (state: ParameterSyncState) => void;
  setCurve: (curve: Segment[]) => void;
  setPresets: (presets: CurvePreset[]) => void;
  resetConnectionData: () => void;
}

const defaultInfo: DeviceInfo = {
  connected: false,
  writeEnabled: false,
  decimalPoint: 1,
  scaleFactor: 1,
};

export const useDeviceStore = create<DeviceState>((set) => ({
  ports: [],
  connectionConfig: {
    port: "",
    slaveAddr: 1,
    baudrate: 9600,
  },
  deviceInfo: defaultInfo,
  latestReading: undefined,
  readings: [],
  error: undefined,
  pid: { p: 0, i: 0, d: 0 },
  setpoint: 100,
  parameterSync: "unknown",
  curve: [{ temperature: 100, minutes: 20 }],
  presets: [],
  setPorts: (ports) =>
    set((state) => ({
      ports,
      connectionConfig: {
        ...state.connectionConfig,
        port: state.connectionConfig.port || ports[0]?.name || "",
      },
    })),
  setConnectionConfig: (config) =>
    set((state) => ({ connectionConfig: { ...state.connectionConfig, ...config } })),
  setDeviceInfo: (deviceInfo) => set({ deviceInfo }),
  setLimits: (limits) => set({ limits }),
  pushReading: (reading) =>
    set((state) => ({
      latestReading: reading,
      readings: [...state.readings, reading].slice(-240),
    })),
  setError: (error) => set({ error }),
  setBackendError: (event) => set({ error: `${event.scope}: ${event.message}` }),
  setPid: (pid) => set({ pid }),
  setSetpoint: (setpoint) => set({ setpoint }),
  setParameterSync: (parameterSync) => set({ parameterSync }),
  setCurve: (curve) => set({ curve }),
  setPresets: (presets) => set({ presets }),
  resetConnectionData: () =>
    set({
      deviceInfo: defaultInfo,
      latestReading: undefined,
      readings: [],
      error: undefined,
      pid: { p: 0, i: 0, d: 0 },
      setpoint: 100,
      parameterSync: "unknown",
    }),
}));
