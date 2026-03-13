export interface DesktopRuntimeInfo {
  expectedProxyPort: number;
  proxyPort: number;
  platform: string;
  startupReady: boolean;
  startupError: string | null;
}

type TauriInvoke = <T>(
  cmd: string,
  args?: Record<string, unknown>,
) => Promise<T>;

type TauriWindowHandle = {
  startDragging(): Promise<void>;
  toggleMaximize(): Promise<void>;
  minimize(): Promise<void>;
  close(): Promise<void>;
  isMaximized(): Promise<boolean>;
};

declare global {
  interface Window {
    __TAURI__?: {
      core?: {
        invoke: TauriInvoke;
      };
      webviewWindow?: {
        getCurrentWebviewWindow(): TauriWindowHandle;
      };
    };
  }
}

let cachedInvoke: TauriInvoke | null = null;
let cachedWindowHandle: TauriWindowHandle | null = null;

function getCurrentInvoke(): TauriInvoke | null {
  const invoke = window.__TAURI__?.core?.invoke;
  if (invoke) {
    cachedInvoke = invoke;
    return invoke;
  }

  return cachedInvoke;
}

export async function invokeDesktop<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  const invoke = getCurrentInvoke();
  if (!invoke) {
    console.error(
      "[desktop-runtime] Tauri invoke bridge is unavailable. Check tauri.conf.json app.withGlobalTauri and desktop runtime injection.",
    );
    throw new Error("Tauri API is not available");
  }
  return invoke<T>(command, args);
}

export async function getDesktopRuntime(): Promise<DesktopRuntimeInfo> {
  return invokeDesktop<DesktopRuntimeInfo>("get_desktop_runtime");
}

export async function updateDesktopProxyPort(
  port: number,
): Promise<DesktopRuntimeInfo> {
  return invokeDesktop<DesktopRuntimeInfo>("update_desktop_proxy_port", {
    port,
  });
}

export function getCurrentDesktopWindow() {
  const currentWindow = window.__TAURI__?.webviewWindow?.getCurrentWebviewWindow();
  if (currentWindow) {
    cachedWindowHandle = currentWindow;
    return currentWindow;
  }

  return cachedWindowHandle;
}
