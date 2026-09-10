import jwt from 'jsonwebtoken';
import { env } from '../config/env';

export interface AccessTokenPayload {
  userId: string;
  email: string;
  role: string;
  /**
   * Session id (`sessions.id`) this access token was minted for.
   *
   * Carrying it lets authenticated requests check that the session is still
   * live — which is what makes "revoke this session" take effect immediately
   * instead of after the access token expires. Optional so tokens issued before
   * this field existed still verify.
   */
  sid?: string;
}

// Refresh tokens are opaque random strings looked up by SHA-256 in the
// sessions table (see utils/tokens.ts) — no JWT signing/verifying needed.

const MS_PER_SECOND = 1_000;
const MS_PER_MINUTE = 60 * MS_PER_SECOND;
const MS_PER_HOUR = 60 * MS_PER_MINUTE;
const MS_PER_DAY = 24 * MS_PER_HOUR;

function parseDurationToMs(duration: string): number {
  const match = duration.match(/^(\d+)(ms|s|m|h|d)$/);
  if (!match) throw new Error(`Invalid duration string: ${duration}`);
  const value = parseInt(match[1]!, 10);
  const unit = match[2]!;
  const multipliers: Record<string, number> = {
    ms: 1,
    s: MS_PER_SECOND,
    m: MS_PER_MINUTE,
    h: MS_PER_HOUR,
    d: MS_PER_DAY,
  };
  return value * multipliers[unit]!;
}

/** TTL in ms for the refresh/session lifetime, per Remember Me choice. */
export function getRefreshTtlMs(rememberMe: boolean): number {
  return parseDurationToMs(rememberMe ? env.JWT_REFRESH_EXPIRES_IN : env.JWT_REFRESH_EXPIRES_IN_SHORT);
}

/** TTL in ms for access tokens (mirrors JWT_ACCESS_EXPIRES_IN). */
export function getAccessTtlMs(): number {
  return parseDurationToMs(env.JWT_ACCESS_EXPIRES_IN);
}

export function getRefreshExpiresAt(rememberMe: boolean): Date {
  return new Date(Date.now() + getRefreshTtlMs(rememberMe));
}

/**
 * Recover the original Remember Me choice from a session row by comparing
 * its TTL against the two configured lifetimes (midpoint threshold).
 */
export function inferRememberMe(createdAt: Date, expiresAt: Date): boolean {
  const ttlMs = expiresAt.getTime() - createdAt.getTime();
  const shortMs = getRefreshTtlMs(false);
  const longMs = getRefreshTtlMs(true);
  return ttlMs > (shortMs + longMs) / 2;
}

export function signAccessToken(payload: AccessTokenPayload): string {
  return jwt.sign(payload, env.JWT_ACCESS_SECRET, {
    expiresIn: env.JWT_ACCESS_EXPIRES_IN,
  } as jwt.SignOptions);
}

export function verifyAccessToken(token: string): AccessTokenPayload {
  return jwt.verify(token, env.JWT_ACCESS_SECRET) as AccessTokenPayload;
}
