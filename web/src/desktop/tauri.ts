export interface DesktopRuntimeInfo {
  expectedProxyPort: number;
  proxyPort: number;
  platform: string;
}

declare global {
  interface Window {
    __TAURI__?: {
      core?: {
        invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T>;
      };
      webviewWindow?: {
        getCurrentWebviewWindow(): {
          startDragging(): Promise<void>;
          toggleMaximize(): Promise<void>;
          minimize(): Promise<void>;
          close(): Promise<void>;
          isMaximized(): Promise<boolean>;
        };
      };
    };
  }
}

export async function invokeDesktop<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  const invoke = window.__TAURI__?.core?.invoke;
  if (!invoke) {
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
  return window.__TAURI__?.webviewWindow?.getCurrentWebviewWindow();
}
