import type { CurvePreset } from "@/lib/types";

const STORE_FILE = "curves.json";
const PRESETS_KEY = "curvePresets";

function hasTauriRuntime() {
  return "__TAURI_INTERNALS__" in window;
}

export async function loadCurvePresets(): Promise<CurvePreset[]> {
  try {
    if (hasTauriRuntime()) {
      const { Store } = await import("@tauri-apps/plugin-store");
      const store = await Store.load(STORE_FILE);
      return ((await store.get<CurvePreset[]>(PRESETS_KEY)) ?? []).filter(Boolean);
    }

    const raw = localStorage.getItem(PRESETS_KEY);
    return raw ? (JSON.parse(raw) as CurvePreset[]) : [];
  } catch (error) {
    console.error("failed to load curve presets", error);
    return [];
  }
}

export async function saveCurvePresets(presets: CurvePreset[]): Promise<void> {
  try {
    if (hasTauriRuntime()) {
      const { Store } = await import("@tauri-apps/plugin-store");
      const store = await Store.load(STORE_FILE);
      await store.set(PRESETS_KEY, presets);
      await store.save();
      return;
    }

    localStorage.setItem(PRESETS_KEY, JSON.stringify(presets));
  } catch (error) {
    console.error("failed to save curve presets", error);
  }
}
