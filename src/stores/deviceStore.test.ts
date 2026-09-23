import { afterEach, describe, expect, it } from "vitest";

import { useDeviceStore } from "./deviceStore";

const connectedInfo = {
  connected: true,
  writeEnabled: true,
  modelCode: 5167,
  modelName: "AI-516P",
  decimalPoint: 2,
  scaleFactor: 1,
};

describe("deviceStore device status", () => {
  afterEach(() => {
    useDeviceStore.setState(useDeviceStore.getInitialState(), true);
  });

  it("adopts the full device info from a connected status event", () => {
    useDeviceStore.getState().applyDeviceStatus(connectedInfo);

    expect(useDeviceStore.getState().deviceInfo).toEqual(connectedInfo);
  });

  it("clears live data but keeps the error when the device drops", () => {
    const store = useDeviceStore.getState();
    store.applyDeviceStatus(connectedInfo);
    store.pushReading({ pv: 100, sv: 100, mv: 50, ts: 1 });
    store.setParameterSync("synced");
    store.setError("connection: link reset");

    store.applyDeviceStatus({ ...connectedInfo, connected: false, writeEnabled: false });

    const state = useDeviceStore.getState();
    expect(state.deviceInfo.connected).toBe(false);
    expect(state.deviceInfo.writeEnabled).toBe(false);
    expect(state.readings).toEqual([]);
    expect(state.latestReading).toBeUndefined();
    expect(state.parameterSync).toBe("unknown");
    expect(state.error).toBe("connection: link reset");
  });
});

describe("deviceStore parameter synchronization", () => {
  it("clears synchronized parameters when connection data resets", () => {
    const store = useDeviceStore.getState();
    store.setDeviceInfo({ connected: true, writeEnabled: true, decimalPoint: 1, scaleFactor: 1 });
    store.setPid({ p: 12, i: 300, d: 4.5 });
    store.setSetpoint(125);
    store.setParameterSync("synced");

    store.resetConnectionData();

    const reset = useDeviceStore.getState();
    expect(reset.deviceInfo.connected).toBe(false);
    expect(reset.parameterSync).toBe("unknown");
    expect(reset.pid).toEqual({ p: 0, i: 0, d: 0 });
    expect(reset.setpoint).toBe(100);
  });
});
