function hasTauriRuntime() {
  return "__TAURI_INTERNALS__" in window;
}

/** Native confirmation dialog in the app; `window.confirm` in the browser mock. */
export async function confirmAction(message: string, title: string): Promise<boolean> {
  if (hasTauriRuntime()) {
    const { confirm } = await import("@tauri-apps/plugin-dialog");
    return confirm(message, { title, kind: "warning" });
  }
  return window.confirm(`${title}\n${message}`);
}
