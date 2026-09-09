/**
 * Per-account lockout-lite: in-memory counter of consecutive failed login
 * attempts keyed by IP + identifier. Resets on successful login.
 *
 * Single-process only (fine for Phase 1); move to Redis when we run multiple
 * instances. Never stores passwords — only identifiers and counters.
 */

const MAX_ATTEMPTS = 5;
const WINDOW_MS = 15 * 60 * 1000; // 15 minutes
const MAX_TRACKED_KEYS = 10_000;

interface AttemptRecord {
  count: number;
  firstAt: number;
}

const attempts = new Map<string, AttemptRecord>();

function key(ip: string | undefined, identifier: string): string {
  return `${ip ?? 'unknown'}|${identifier.toLowerCase()}`;
}

function pruneExpired(): void {
  const now = Date.now();
  for (const [k, v] of attempts) {
    if (now - v.firstAt > WINDOW_MS) attempts.delete(k);
  }
}

export function isThrottled(ip: string | undefined, identifier: string): boolean {
  const record = attempts.get(key(ip, identifier));
  if (!record) return false;
  if (Date.now() - record.firstAt > WINDOW_MS) {
    attempts.delete(key(ip, identifier));
    return false;
  }
  return record.count >= MAX_ATTEMPTS;
}

export function recordFailedAttempt(ip: string | undefined, identifier: string): void {
  if (attempts.size > MAX_TRACKED_KEYS) pruneExpired();
  const k = key(ip, identifier);
  const record = attempts.get(k);
  if (!record || Date.now() - record.firstAt > WINDOW_MS) {
    attempts.set(k, { count: 1, firstAt: Date.now() });
  } else {
    attempts.set(k, { ...record, count: record.count + 1 });
  }
}

export function clearFailedAttempts(ip: string | undefined, identifier: string): void {
  attempts.delete(key(ip, identifier));
}
