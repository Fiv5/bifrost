const SECOND_IN_MS = 1000;
const MINUTE_IN_MS = 60 * SECOND_IN_MS;
const HOUR_IN_MS = 60 * MINUTE_IN_MS;

const trimTrailingZero = (value: number): string =>
  value.toFixed(1).replace(/\.0$/, "");

export function formatDurationCompact(ms?: number | null): string {
  if (ms === undefined || ms === null || ms <= 0) return "-";
  if (ms < SECOND_IN_MS) return `${Math.round(ms)}ms`;
  if (ms < MINUTE_IN_MS) return `${trimTrailingZero(ms / SECOND_IN_MS)}s`;
  if (ms < HOUR_IN_MS) return `${trimTrailingZero(ms / MINUTE_IN_MS)}m`;
  return `${trimTrailingZero(ms / HOUR_IN_MS)}h`;
}

export function formatDurationDetailed(ms?: number | null): string {
  if (ms === undefined || ms === null || ms <= 0) return "-";
  if (ms < SECOND_IN_MS) return `${Math.round(ms)}ms`;

  let remaining = Math.round(ms);
  const hours = Math.floor(remaining / HOUR_IN_MS);
  remaining %= HOUR_IN_MS;
  const minutes = Math.floor(remaining / MINUTE_IN_MS);
  remaining %= MINUTE_IN_MS;
  const seconds = Math.floor(remaining / SECOND_IN_MS);
  remaining %= SECOND_IN_MS;

  const parts: string[] = [];
  if (hours > 0) parts.push(`${hours}h`);
  if (minutes > 0) parts.push(`${minutes}m`);
  if (seconds > 0) parts.push(`${seconds}s`);
  if (remaining > 0) parts.push(`${remaining}ms`);

  return parts.join(" ");
}
