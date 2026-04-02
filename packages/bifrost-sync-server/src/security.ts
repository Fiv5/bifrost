export interface RateLimitEntry {
  count: number;
  resetAt: number;
}

export class RateLimiter {
  private buckets = new Map<string, RateLimitEntry>();
  private cleanupTimer: ReturnType<typeof setInterval>;

  constructor(
    private maxRequests: number,
    private windowMs: number,
  ) {
    this.cleanupTimer = setInterval(() => this.cleanup(), Math.max(windowMs, 60_000));
  }

  check(key: string): { allowed: boolean; remaining: number; retryAfterMs: number } {
    const now = Date.now();
    const entry = this.buckets.get(key);

    if (!entry || now >= entry.resetAt) {
      this.buckets.set(key, { count: 1, resetAt: now + this.windowMs });
      return { allowed: true, remaining: this.maxRequests - 1, retryAfterMs: 0 };
    }

    if (entry.count >= this.maxRequests) {
      return { allowed: false, remaining: 0, retryAfterMs: entry.resetAt - now };
    }

    entry.count++;
    return { allowed: true, remaining: this.maxRequests - entry.count, retryAfterMs: 0 };
  }

  private cleanup() {
    const now = Date.now();
    for (const [key, entry] of this.buckets) {
      if (now >= entry.resetAt) this.buckets.delete(key);
    }
  }

  destroy() {
    clearInterval(this.cleanupTimer);
    this.buckets.clear();
  }
}

export interface AccountLockEntry {
  failures: number;
  lockedUntil: number;
  lastFailure: number;
}

export class AccountLockManager {
  private locks = new Map<string, AccountLockEntry>();
  private cleanupTimer: ReturnType<typeof setInterval>;

  constructor(
    private maxFailures: number = 5,
    private lockDurationMs: number = 15 * 60 * 1000,
    private failureWindowMs: number = 30 * 60 * 1000,
  ) {
    this.cleanupTimer = setInterval(() => this.cleanup(), 60_000);
  }

  isLocked(userId: string): { locked: boolean; retryAfterMs: number } {
    const entry = this.locks.get(userId);
    if (!entry) return { locked: false, retryAfterMs: 0 };

    const now = Date.now();
    if (entry.lockedUntil > now) {
      return { locked: true, retryAfterMs: entry.lockedUntil - now };
    }

    if (now - entry.lastFailure > this.failureWindowMs) {
      this.locks.delete(userId);
      return { locked: false, retryAfterMs: 0 };
    }

    return { locked: false, retryAfterMs: 0 };
  }

  recordFailure(userId: string): { locked: boolean; failures: number } {
    const now = Date.now();
    const entry = this.locks.get(userId);

    if (!entry || now - entry.lastFailure > this.failureWindowMs) {
      this.locks.set(userId, { failures: 1, lockedUntil: 0, lastFailure: now });
      return { locked: false, failures: 1 };
    }

    entry.failures++;
    entry.lastFailure = now;

    if (entry.failures >= this.maxFailures) {
      entry.lockedUntil = now + this.lockDurationMs;
      console.warn(`[bifrost-sync-server] account locked: ${userId} (${entry.failures} failures)`);
      return { locked: true, failures: entry.failures };
    }

    return { locked: false, failures: entry.failures };
  }

  recordSuccess(userId: string) {
    this.locks.delete(userId);
  }

  private cleanup() {
    const now = Date.now();
    for (const [key, entry] of this.locks) {
      if (now - entry.lastFailure > this.failureWindowMs && entry.lockedUntil <= now) {
        this.locks.delete(key);
      }
    }
  }

  destroy() {
    clearInterval(this.cleanupTimer);
    this.locks.clear();
  }
}

const USERNAME_RE = /^[a-zA-Z0-9_\-@.]{2,64}$/;

export function validateUsername(userId: string): string | null {
  if (!userId || typeof userId !== 'string') return 'username is required';
  if (userId.length < 2) return 'username must be at least 2 characters';
  if (userId.length > 64) return 'username must be at most 64 characters';
  if (!USERNAME_RE.test(userId)) return 'username can only contain letters, numbers, _ - @ .';
  return null;
}

export function validatePassword(password: string): string | null {
  if (!password || typeof password !== 'string') return 'password is required';
  if (password.length < 6) return 'password must be at least 6 characters';
  if (password.length > 128) return 'password must be at most 128 characters';
  return null;
}

export function getClientIp(req: { headers: Record<string, string | string[] | undefined>; socket?: { remoteAddress?: string } }): string {
  const xff = req.headers['x-forwarded-for'];
  if (xff) {
    const first = (Array.isArray(xff) ? xff[0] : xff).split(',')[0].trim();
    if (first) return first;
  }
  const xRealIp = req.headers['x-real-ip'];
  if (xRealIp) {
    return Array.isArray(xRealIp) ? xRealIp[0] : xRealIp;
  }
  return req.socket?.remoteAddress ?? 'unknown';
}
