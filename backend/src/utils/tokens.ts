import crypto from 'crypto';

/**
 * Opaque token utilities (refresh tokens, password-reset tokens).
 *
 * Tokens are random, high-entropy strings. We persist a deterministic
 * SHA-256 hex digest so the DB row can be found by exact lookup
 * (unique index) while a stolen DB dump cannot be replayed.
 */

export function generateOpaqueToken(): string {
  return crypto.randomBytes(48).toString('base64url');
}

export function sha256Hex(value: string): string {
  return crypto.createHash('sha256').update(value, 'utf8').digest('hex');
}
