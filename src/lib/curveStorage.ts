import { z } from "zod";

import type { CurvePreset } from "@/lib/types";
import type { ValidationLimits } from "@/lib/types";
import { createCurveSchema } from "@/lib/validation";

const STORE_FILE = "curves.json";
const PRESETS_KEY = "curvePresets";

function hasTauriRuntime() {
  return "__TAURI_INTERNALS__" in window;
}

function parseCurvePresets(raw: unknown, limits: ValidationLimits): CurvePreset[] {
  if (!Array.isArray(raw)) {
    console.warn("stored curve presets are not an array");
    return [];
  }

  const schema = z.object({
    id: z.string().min(1),
    name: z.string().min(1),
    description: z.string(),
    segments: createCurveSchema(limits),
  });

  return raw.flatMap((candidate, index) => {
    const parsed = schema.safeParse(candidate);
    if (!parsed.success) {
      console.warn(`discarding invalid curve preset at index ${index}`);
      return [];
    }
    return [parsed.data];
  });
}

export async function loadCurvePresets(limits: ValidationLimits): Promise<CurvePreset[]> {
  try {
    if (hasTauriRuntime()) {
      const { Store } = await import("@tauri-apps/plugin-store");
      const store = await Store.load(STORE_FILE);
      return parseCurvePresets(await store.get<unknown>(PRESETS_KEY), limits);
    }

    const raw = localStorage.getItem(PRESETS_KEY);
    return raw ? parseCurvePresets(JSON.parse(raw), limits) : [];
  } catch (error) {
    console.error("failed to load curve presets", error);
    return [];
  }
}

export async function saveCurvePresets(
  presets: CurvePreset[],
  limits: ValidationLimits,
): Promise<void> {
  try {
    const validPresets = parseCurvePresets(presets, limits);
    if (hasTauriRuntime()) {
      const { Store } = await import("@tauri-apps/plugin-store");
      const store = await Store.load(STORE_FILE);
      await store.set(PRESETS_KEY, validPresets);
      await store.save();
      return;
    }

    localStorage.setItem(PRESETS_KEY, JSON.stringify(validPresets));
  } catch (error) {
    console.error("failed to save curve presets", error);
  }
}
