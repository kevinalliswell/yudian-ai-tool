import { afterEach, describe, expect, it, vi } from "vitest";

import { mockApi } from "./mockApi";

describe("mockApi", () => {
  afterEach(async () => {
    await mockApi.disconnect().catch(() => undefined);
    vi.useRealTimers();
  });

  it("runs scan connect monitor skeleton", async () => {
    vi.useFakeTimers();
    const ports = await mockApi.listSerialPorts();
    expect(ports[0]?.name).toBe("COM_MOCK");

    const readings: number[] = [];
    const unlisten = await mockApi.onReading((payload) => {
      if (typeof payload.pv === "number") readings.push(payload.pv);
    });

    const info = await mockApi.connect({ port: "COM_MOCK", slaveAddr: 1, baudrate: 9600 });
    expect(info.modelName).toBe("AI-516P");

    await mockApi.startMonitoring(200);
    vi.advanceTimersByTime(250);
    expect(readings.length).toBeGreaterThan(0);

    unlisten();
  });
});
