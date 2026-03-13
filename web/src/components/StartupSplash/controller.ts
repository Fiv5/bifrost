export const STARTUP_SPLASH_EXIT_MS = 900;
export const STARTUP_SPLASH_EVENT = "bifrost:startup-splash-exit";

export function beginStartupSplashExit() {
  window.dispatchEvent(new Event(STARTUP_SPLASH_EVENT));
}
