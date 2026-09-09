import * as argon2 from 'argon2';

const ARGON2_OPTIONS = {
  type: argon2.argon2id,
  memoryCost: 19 * 1024, // 19 MiB
  timeCost: 2,
  parallelism: 1,
} as const;

export async function hashPassword(plain: string): Promise<string> {
  return argon2.hash(plain, ARGON2_OPTIONS);
}

export async function verifyPassword(hash: string, plain: string): Promise<boolean> {
  try {
    return await argon2.verify(hash, plain);
  } catch {
    return false;
  }
}

/**
 * Constant bogus hash (argon2id, same params as ARGON2_OPTIONS) used to
 * equalize response timing between "unknown account" and "wrong password".
 * Without this, unknown-account fails instantly and attackers can enumerate
 * roll numbers / emails from response times.
 *
 * Generated once from a random throwaway value; this password is not a secret.
 */
const DUMMY_PASSWORD_HASH =
  '$argon2id$v=19$m=19456,t=2,p=1$NMhPUTWMwDwkTOXTiJX7+w$Nw+kV8ZZN6qNirY9lGOPjUifAikFJlwEAT6HCT47Mew';

/** Runs a real argon2 verification against the dummy hash — costs the same as a wrong-password check. */
export async function burnPasswordVerify(): Promise<void> {
  await verifyPassword(DUMMY_PASSWORD_HASH, 'timing-equalization-dummy-never-a-real-password');
}
