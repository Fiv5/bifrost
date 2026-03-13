import { useEffect, useState } from "react";
import {
  STARTUP_SPLASH_EVENT,
  STARTUP_SPLASH_EXIT_MS,
} from "./controller";

interface StartupSplashProps {
  phase?: "visible" | "exiting";
}

export default function StartupSplash({
  phase = "visible",
}: StartupSplashProps) {
  return (
    <div
      className={`startup-shell ${phase === "exiting" ? "startup-shell--exiting" : ""}`}
      aria-label="Bifrost startup screen"
    >
      <div className="startup-panel">
        <div className="startup-icon-wrap" aria-hidden="true">
          <svg className="startup-heartbeat" viewBox="0 0 96 96">
            <path
              className="startup-line"
              d="M12 72h16l7-10 7 18 10-30 8 22 8-10h16"
            />
            <path
              className="startup-heart"
              d="M48 25c5.8-8.1 17.7-9.1 24.3-1.9 6.3 6.8 6.1 17.4-0.2 24.1L48 71 23.9 47.2c-6.4-6.7-6.5-17.3-0.2-24.1C30.3 15.9 42.2 16.9 48 25Z"
            />
          </svg>
        </div>
        <p className="startup-subtitle">Bifrost Proxy</p>
        <p className="startup-status">
          Starting<span className="startup-dots" aria-hidden="true"></span>
        </p>
      </div>
    </div>
  );
}

export function StartupSplashHost() {
  const [phase, setPhase] = useState<"visible" | "exiting">("visible");
  const [mounted, setMounted] = useState(true);

  useEffect(() => {
    let timer: number | null = null;

    const handleExit = () => {
      setPhase("exiting");
      if (timer !== null) {
        window.clearTimeout(timer);
      }
      timer = window.setTimeout(() => {
        setMounted(false);
      }, STARTUP_SPLASH_EXIT_MS);
    };

    window.addEventListener(STARTUP_SPLASH_EVENT, handleExit);
    return () => {
      window.removeEventListener(STARTUP_SPLASH_EVENT, handleExit);
      if (timer !== null) {
        window.clearTimeout(timer);
      }
    };
  }, []);

  if (!mounted) {
    return null;
  }

  return <StartupSplash phase={phase} />;
}
