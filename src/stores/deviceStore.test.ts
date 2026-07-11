import { describe, expect, it } from "vitest";

import { useDeviceStore } from "./deviceStore";

describe("deviceStore parameter synchronization", () => {
  it("clears synchronized parameters when connection data resets", () => {
    const store = useDeviceStore.getState();
    store.setDeviceInfo({ connected: true, decimalPoint: 1, scaleFactor: 1 });
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
