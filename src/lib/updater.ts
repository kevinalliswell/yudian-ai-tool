function hasTauriRuntime() {
  return "__TAURI_INTERNALS__" in window;
}

export interface AvailableUpdate {
  version: string;
  currentVersion: string;
  notes?: string;
  /** Downloads, installs and relaunches; progress is 0..1 when the size is known. */
  install: (onProgress: (fraction: number | undefined) => void) => Promise<void>;
}

/** Updates come from signed GitHub releases, so only the desktop app can apply them. */
export function updatesSupported() {
  return hasTauriRuntime();
}

export async function checkForUpdate(): Promise<AvailableUpdate | null> {
  if (!updatesSupported()) return null;
  const { check } = await import("@tauri-apps/plugin-updater");
  const update = await check();
  if (!update) return null;

  return {
    version: update.version,
    currentVersion: update.currentVersion,
    notes: update.body,
    async install(onProgress) {
      let total: number | undefined;
      let received = 0;
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          total = event.data.contentLength;
          onProgress(total ? 0 : undefined);
        } else if (event.event === "Progress") {
          received += event.data.chunkLength;
          onProgress(total ? Math.min(received / total, 1) : undefined);
        } else {
          onProgress(1);
        }
      });
      // Windows exits into the installer; other platforms need a relaunch.
      const { relaunch } = await import("@tauri-apps/plugin-process");
      await relaunch();
    },
  };
}
