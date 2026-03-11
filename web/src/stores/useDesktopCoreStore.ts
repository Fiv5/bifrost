import { create } from "zustand";

export type DesktopCorePhase =
  | "idle"
  | "booting"
  | "saving"
  | "restarting"
  | "reconnecting"
  | "error";

interface DesktopCoreState {
  visible: boolean;
  phase: DesktopCorePhase;
  targetPort: number | null;
  detail: string;
  readyOnce: boolean;
  beginRestart: (port: number) => void;
  showBooting: (detail?: string) => void;
  markReady: () => void;
  setPhase: (phase: Exclude<DesktopCorePhase, "idle" | "error">, detail?: string) => void;
  failRestart: (detail: string) => void;
  resolveBooting: () => void;
  hide: () => void;
}

export const useDesktopCoreStore = create<DesktopCoreState>((set) => ({
  visible: false,
  phase: "idle",
  targetPort: null,
  detail: "",
  readyOnce: false,
  beginRestart: (port) =>
    set({
      visible: true,
      phase: "saving",
      targetPort: port,
      detail: `Preparing proxy core restart on port ${port}`,
      readyOnce: false,
    }),
  showBooting: (detail) =>
    set((state) => ({
      visible: true,
      phase: state.phase === "restarting" || state.phase === "reconnecting"
        ? state.phase
        : "booting",
      targetPort: state.targetPort,
      detail: detail ?? "Bifrost core is starting. Reconnecting the interface...",
      readyOnce: state.readyOnce,
    })),
  markReady: () =>
    set((state) => ({
      readyOnce: true,
      visible: state.phase === "booting" ? false : state.visible,
      phase: state.phase === "booting" ? "idle" : state.phase,
      targetPort: state.phase === "booting" ? null : state.targetPort,
      detail: state.phase === "booting" ? "" : state.detail,
    })),
  setPhase: (phase, detail) =>
    set((state) => ({
      visible: true,
      phase,
      targetPort: state.targetPort,
      detail: detail ?? state.detail,
      readyOnce: state.readyOnce,
    })),
  failRestart: (detail) =>
    set((state) => ({
      visible: true,
      phase: "error",
      targetPort: state.targetPort,
      detail,
      readyOnce: state.readyOnce,
    })),
  resolveBooting: () =>
    set((state) =>
      state.phase === "booting"
        ? {
            visible: false,
            phase: "idle",
            targetPort: null,
            detail: "",
            readyOnce: true,
          }
        : state,
    ),
  hide: () =>
    set({
      visible: false,
      phase: "idle",
      targetPort: null,
      detail: "",
      readyOnce: true,
    }),
}));

export function isDesktopCoreTransitionActive(): boolean {
  const { visible, phase } = useDesktopCoreStore.getState();
  return visible && phase !== "idle" && phase !== "error";
}
