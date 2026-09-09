import rateLimit from 'express-rate-limit';
import { env } from '../config/env';

const RATE_LIMIT_HEADERS = { standardHeaders: true, legacyHeaders: false } as const;

// General API rate limiter
export const apiRateLimiter = rateLimit({
  windowMs: env.RATE_LIMIT_WINDOW_MS,
  max: env.RATE_LIMIT_MAX_REQUESTS,
  ...RATE_LIMIT_HEADERS,
  message: { success: false, error: 'Too many requests, please try again later' },
});

// Stricter limiter for login endpoints — per-IP brute-force mitigation.
// (Complements per-account lockout-lite in utils/loginThrottle.ts)
export const loginRateLimiter = rateLimit({
  windowMs: 15 * 60 * 1000,
  max: 20,
  ...RATE_LIMIT_HEADERS,
  message: { success: false, error: 'Too many login attempts, please try again after 15 minutes' },
  skipSuccessfulRequests: false,
});

export const signupRateLimiter = rateLimit({
  windowMs: 60 * 60 * 1000,
  max: 10,
  ...RATE_LIMIT_HEADERS,
  message: { success: false, error: 'Too many signup attempts, please try again later' },
});

export const refreshRateLimiter = rateLimit({
  windowMs: 15 * 60 * 1000,
  max: 60,
  ...RATE_LIMIT_HEADERS,
  message: { success: false, error: 'Too many refresh attempts, please try again later' },
});

export const forgotPasswordRateLimiter = rateLimit({
  windowMs: 15 * 60 * 1000,
  max: 10,
  ...RATE_LIMIT_HEADERS,
  message: { success: false, error: 'Too many password reset attempts, please try again later' },
});

// Change-password verifies a current password — same brute-force surface as
// login, so it gets an equally strict limiter.
export const passwordChangeRateLimiter = rateLimit({
  windowMs: 15 * 60 * 1000,
  max: 10,
  ...RATE_LIMIT_HEADERS,
  message: { success: false, error: 'Too many password change attempts, please try again later' },
});

export default loginRateLimiter;
