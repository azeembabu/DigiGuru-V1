import type { CookieOptions, Response } from 'express';
import { env } from '../config/env';
import { getAccessTtlMs } from './jwt';

export const ACCESS_TOKEN_COOKIE = 'access_token';
export const REFRESH_TOKEN_COOKIE = 'refresh_token';

function baseOptions(): CookieOptions {
  const isProd = env.NODE_ENV === 'production';
  // Browsers reject cookies with Domain=localhost, so omit it for local hosts.
  const usableDomain =
    env.COOKIE_DOMAIN && !['localhost', '127.0.0.1'].includes(env.COOKIE_DOMAIN)
      ? env.COOKIE_DOMAIN
      : undefined;

  return {
    httpOnly: true,
    secure: isProd || env.COOKIE_SECURE,
    sameSite: 'lax',
    path: '/',
    ...(usableDomain ? { domain: usableDomain } : {}),
  };
}

/**
 * Sets access + refresh cookies. Refresh cookie expiry is driven by the
 * session's actual DB expiry (Remember Me aware) so cookie and token never drift.
 */
export function setAuthCookies(
  res: Response,
  accessToken: string,
  refreshToken: string,
  refreshExpiresAt: Date,
): void {
  res.cookie(ACCESS_TOKEN_COOKIE, accessToken, {
    ...baseOptions(),
    maxAge: getAccessTtlMs(),
  });

  res.cookie(REFRESH_TOKEN_COOKIE, refreshToken, {
    ...baseOptions(),
    expires: refreshExpiresAt,
  });
}

export function clearAuthCookies(res: Response): void {
  res.clearCookie(ACCESS_TOKEN_COOKIE, baseOptions());
  res.clearCookie(REFRESH_TOKEN_COOKIE, baseOptions());
}
