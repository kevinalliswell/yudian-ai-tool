import { z } from "zod";

import type { ConnectionConfig, DeviceInfo } from "@/lib/types";

const STORE_FILE = "audit.json";
const ENTRIES_KEY = "auditEntries";
const MAX_ENTRIES = 500;

export type AuditAction =
  | "connect"
  | "disconnect"
  | "write_setpoint"
  | "write_pid"
  | "run_status"
  | "curve_upload"
  | "curve_download"
  | "preset_save";

export type AuditOutcome = "success" | "failure" | "rejected";

export type RollbackStatus = "not_applicable" | "succeeded" | "failed" | "unknown";

export interface AuditDetails {
  connection?: ConnectionConfig;
  device?: DeviceInfo;
  before?: unknown;
  after?: unknown;
  attempted?: unknown;
  status?: string;
  error?: string;
  rollback?: RollbackStatus;
  reason?: string;
}

export interface AuditEntry {
  id: string;
  timestamp: string;
  action: AuditAction;
  outcome: AuditOutcome;
  details: AuditDetails;
}

const auditEntrySchema = z.object({
  id: z.string().min(1),
  timestamp: z.string().min(1),
  action: z.enum([
    "connect",
    "disconnect",
    "write_setpoint",
    "write_pid",
    "run_status",
    "curve_upload",
    "curve_download",
    "preset_save",
  ]),
  outcome: z.enum(["success", "failure", "rejected"]),
  details: z.record(z.string(), z.unknown()),
});

const auditEntriesSchema = z.array(auditEntrySchema);
let writeQueue = Promise.resolve();

function hasTauriRuntime() {
  return "__TAURI_INTERNALS__" in window;
}

async function readEntries(): Promise<AuditEntry[]> {
  try {
    const raw = hasTauriRuntime()
      ? await (async () => {
          const { Store } = await import("@tauri-apps/plugin-store");
          const store = await Store.load(STORE_FILE);
          return store.get<unknown>(ENTRIES_KEY);
        })()
      : JSON.parse(localStorage.getItem(ENTRIES_KEY) ?? "null");
    const parsed = auditEntriesSchema.safeParse(raw);
    return parsed.success ? parsed.data : [];
  } catch (error) {
    console.error("failed to load audit entries", error);
    return [];
  }
}

async function writeEntries(entries: AuditEntry[]) {
  try {
    if (hasTauriRuntime()) {
      const { Store } = await import("@tauri-apps/plugin-store");
      const store = await Store.load(STORE_FILE);
      await store.set(ENTRIES_KEY, entries);
      await store.save();
      return;
    }

    localStorage.setItem(ENTRIES_KEY, JSON.stringify(entries));
  } catch (error) {
    console.error("failed to save audit entries", error);
  }
}

export async function loadAuditEntries() {
  return readEntries();
}

export function recordAuditEvent(
  event: Omit<AuditEntry, "id" | "timestamp"> & { id?: string; timestamp?: string },
) {
  const entry: AuditEntry = {
    ...event,
    id: event.id ?? crypto.randomUUID(),
    timestamp: event.timestamp ?? new Date().toISOString(),
  };

  writeQueue = writeQueue.then(async () => {
    const entries = await readEntries();
    await writeEntries([...entries, entry].slice(-MAX_ENTRIES));
  });
  return writeQueue;
}

export function rollbackStatusFromError(error: unknown): RollbackStatus {
  const message = error instanceof Error ? error.message : String(error);
  if (message.includes("rollback succeeded")) return "succeeded";
  if (message.includes("rollback failed")) return "failed";
  return "unknown";
}
