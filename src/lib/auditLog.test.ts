import { afterEach, describe, expect, it } from "vitest";

import { loadAuditEntries, recordAuditEvent, rollbackStatusFromError } from "./auditLog";

describe("auditLog", () => {
  afterEach(() => {
    localStorage.clear();
  });

  it("persists structured audit entries in the frontend runtime", async () => {
    await recordAuditEvent({
      action: "write_setpoint",
      outcome: "success",
      details: {
        before: 100,
        after: 120,
        rollback: "not_applicable",
      },
    });

    await expect(loadAuditEntries()).resolves.toMatchObject([
      {
        action: "write_setpoint",
        outcome: "success",
        details: { before: 100, after: 120 },
      },
    ]);
  });

  it("serializes concurrent writes without losing entries", async () => {
    await Promise.all([
      recordAuditEvent({
        action: "run_status",
        outcome: "success",
        details: { status: "run" },
      }),
      recordAuditEvent({
        action: "run_status",
        outcome: "failure",
        details: { status: "stop", error: "device error" },
      }),
    ]);

    const entries = await loadAuditEntries();
    expect(entries).toHaveLength(2);
    expect(entries.map((entry) => entry.outcome)).toEqual(["success", "failure"]);
  });

  it("detects rollback results from serialized backend errors", () => {
    expect(rollbackStatusFromError({ message: "rollback succeeded" })).toBe("succeeded");
    expect(rollbackStatusFromError({ message: "rollback failed: timeout" })).toBe("failed");
  });
});
