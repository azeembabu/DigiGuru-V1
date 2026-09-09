import dotenv from 'dotenv';

dotenv.config();

const PLACEHOLDER_PATTERNS = ['change-me', 'secret-change-me', 'your-secret', 'placeholder'];
const MIN_SECRET_LENGTH = 32;

function requireEnv(name: string, fallback?: string): string {
  const value = process.env[name] ?? fallback;
  if (!value) {
    throw new Error(`Missing required env variable: ${name}`);
  }
  return value;
}

/**
 * Fail fast on weak/placeholder JWT secrets. In production any placeholder or
 * short secret is fatal; in development a placeholder is fatal too (the dev
 * fallbacks below are strong random-length strings, so only a real
 * "change-me" left in .env triggers this).
 */
function validateSecret(name: string, value: string): string {
  const lower = value.toLowerCase();
  if (PLACEHOLDER_PATTERNS.some((p) => lower.includes(p))) {
    throw new Error(`Insecure ${name}: placeholder value detected. Generate one with: node -e "console.log(require('crypto').randomBytes(64).toString('hex'))"`);
  }
  if (value.length < MIN_SECRET_LENGTH) {
    throw new Error(`Insecure ${name}: must be at least ${MIN_SECRET_LENGTH} characters (got ${value.length}).`);
  }
  return value;
}

export const env = {
  NODE_ENV: process.env.NODE_ENV ?? 'development',
  PORT: parseInt(process.env.PORT ?? '4000', 10),
  API_PREFIX: process.env.API_PREFIX ?? '/api',
  DATABASE_URL: requireEnv('DATABASE_URL', 'postgresql://digiguru:digiguru_dev_password@localhost:5432/digiguru_dev'),

  JWT_ACCESS_SECRET: validateSecret('JWT_ACCESS_SECRET', requireEnv('JWT_ACCESS_SECRET')),
  JWT_REFRESH_SECRET: validateSecret('JWT_REFRESH_SECRET', requireEnv('JWT_REFRESH_SECRET')),
  JWT_ACCESS_EXPIRES_IN: process.env.JWT_ACCESS_EXPIRES_IN ?? '15m',
  JWT_REFRESH_EXPIRES_IN: process.env.JWT_REFRESH_EXPIRES_IN ?? '30d',
  JWT_REFRESH_EXPIRES_IN_SHORT: process.env.JWT_REFRESH_EXPIRES_IN_SHORT ?? '8h',

  CORS_ORIGINS: (process.env.CORS_ORIGINS ?? 'http://localhost:3000,http://localhost:3001')
    .split(',')
    .map((o) => o.trim())
    .filter(Boolean),

  RATE_LIMIT_WINDOW_MS: parseInt(process.env.RATE_LIMIT_WINDOW_MS ?? '900000', 10),
  RATE_LIMIT_MAX_REQUESTS: parseInt(process.env.RATE_LIMIT_MAX_REQUESTS ?? '100', 10),

  COOKIE_DOMAIN: process.env.COOKIE_DOMAIN,
  COOKIE_SECURE: process.env.COOKIE_SECURE === 'true',

  LOG_LEVEL: process.env.LOG_LEVEL ?? 'info',
} as const;

export const isProduction = env.NODE_ENV === 'production';
export const isDevelopment = env.NODE_ENV === 'development';
export const isTest = env.NODE_ENV === 'test';
export type Env = typeof env;
