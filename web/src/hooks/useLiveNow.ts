import { useEffect, useState } from "react";

export function useLiveNow(active: boolean, intervalMs = 500): number {
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    if (!active) {
      return;
    }

    const update = () => setNow(Date.now());
    const timer = window.setInterval(update, intervalMs);
    const immediate = window.setTimeout(update, 0);

    return () => {
      window.clearTimeout(immediate);
      window.clearInterval(timer);
    };
  }, [active, intervalMs]);

  return now;
}
