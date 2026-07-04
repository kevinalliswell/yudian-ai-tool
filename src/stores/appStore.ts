import { create } from "zustand";

export type ShellSection = "connection" | "monitor" | "parameters" | "curves";

interface AppState {
  activeSection: ShellSection;
  setActiveSection: (section: ShellSection) => void;
}

export const useAppStore = create<AppState>((set) => ({
  activeSection: "connection",
  setActiveSection: (activeSection) => set({ activeSection }),
}));
