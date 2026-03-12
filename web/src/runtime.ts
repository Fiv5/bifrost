const DEFAULT_ADMIN_PREFIX = '/_bifrost';
const DEFAULT_BACKEND_PORT = 9900;

type DesktopPlatform = 'macos' | 'windows' | 'linux' | 'web';

const desktopRuntime = {
  initialized: false,
  expectedProxyPort: DEFAULT_BACKEND_PORT,
  proxyPort: DEFAULT_BACKEND_PORT,
  platform: 'web' as DesktopPlatform,
};

export function isDesktopShell(): boolean {
  return import.meta.env.MODE === 'desktop';
}

export function getDesktopPlatform(): DesktopPlatform {
  return desktopRuntime.platform;
}

export function setDesktopProxyPort(port: number): void {
  desktopRuntime.proxyPort = port;
}

export function getExpectedDesktopProxyPort(): number {
  return desktopRuntime.expectedProxyPort;
}

export async function initializeDesktopRuntime(): Promise<void> {
  if (!isDesktopShell() || desktopRuntime.initialized) {
    desktopRuntime.initialized = true;
    return;
  }

  try {
    const { getDesktopRuntime } = await import('./desktop/tauri');
    const runtime = await getDesktopRuntime();
    desktopRuntime.expectedProxyPort = runtime.expectedProxyPort;
    desktopRuntime.proxyPort = runtime.proxyPort;
    desktopRuntime.platform =
      runtime.platform === 'darwin'
        ? 'macos'
        : runtime.platform === 'win32'
          ? 'windows'
          : runtime.platform === 'linux'
            ? 'linux'
            : 'web';
  } finally {
    desktopRuntime.initialized = true;
  }
}

export function getAdminPrefix(): string {
  return DEFAULT_ADMIN_PREFIX;
}

export function getBackendOrigin(): string {
  if (!isDesktopShell()) {
    return window.location.origin;
  }

  return `http://127.0.0.1:${desktopRuntime.proxyPort}`;
}

export function buildBackendUrl(path: string): string {
  const normalizedPath = path.startsWith('/') ? path : `/${path}`;
  const adminPrefix = getAdminPrefix();
  const resolvedPath = normalizedPath.startsWith(adminPrefix)
    ? normalizedPath
    : `${adminPrefix}${normalizedPath}`;
  return `${getBackendOrigin()}${resolvedPath}`;
}

export function buildApiUrl(path = ''): string {
  const suffix = path.startsWith('/') ? path : `/${path}`;
  return buildBackendUrl(`/api${suffix === '/' ? '' : suffix}`);
}

export function buildPublicUrl(path = ''): string {
  const suffix = path.startsWith('/') ? path : `/${path}`;
  return buildBackendUrl(`/public${suffix === '/' ? '' : suffix}`);
}

export function buildWsUrl(path: string, params?: URLSearchParams): string {
  const url = new URL(buildBackendUrl(path));
  url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
  if (params) {
    url.search = params.toString();
  }
  return url.toString();
}

export function resolveRequestUrl(input: RequestInfo | URL): RequestInfo | URL {
  if (typeof input !== 'string') {
    return input;
  }

  if (/^[a-zA-Z][a-zA-Z\d+\-.]*:/.test(input)) {
    return input;
  }

  if (input.startsWith('/')) {
    return buildBackendUrl(input);
  }

  return input;
}
