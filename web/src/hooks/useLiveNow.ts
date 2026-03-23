import { useEffect, useState } from "react";

export function useLiveNow(active: boolean, intervalMs = 500): number {
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    if (!active) {
      return;
    }

    setNow(Date.now());
    const timer = window.setInterval(() => {
      setNow(Date.now());
    }, intervalMs);

    return () => {
      window.clearInterval(timer);
    };
  }, [active, intervalMs]);

  return now;
}
