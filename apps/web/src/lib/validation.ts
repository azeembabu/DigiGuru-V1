// Client-side mirrors of the gateway's own validators. These exist to give
// instant feedback, never to be the authority — the server re-validates
// everything and its message wins on conflict.
//
// Each rule below is deliberately no STRICTER than the server's, so the form
// can never reject a value the gateway would have accepted:
//   roll_number  see ROLL_NUMBER_RE below   apps/gateway/src/validation.rs
//   phone_number 10-digit Indian mobile   apps/gateway/src/validation.rs
//   password     8-128 characters         apps/gateway/src/auth/signup.rs
//   full_name    2-120 characters         apps/gateway/src/auth/signup.rs

export type FieldErrors = Record<string, string>;

// YY + intake + level + 2-letter programme code + 5-digit serial, e.g.
// 25XHBML11450 = 2025, first intake (X), honours bachelor (HB), Malayalam
// (ML), serial 11450. Level is HB (honours bachelor), B (bachelor) or M
// (masters); honours is bachelor-only, so there is no HM. Mirrors
// ROLL_NUMBER_RE in apps/gateway/src/validation.rs.
const ROLL_NUMBER_RE = /^\d{2}[XY](?:HB|B|M)[A-Z]{2}\d{5}$/;
const INDIAN_MOBILE_RE = /^[6-9]\d{9}$/;
// Deliberately permissive: the gateway uses a full RFC-validating email
// crate, and a stricter client regex would reject addresses it accepts.
const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

/**
 * Reduce a phone number to its 10 significant digits, accepting the same
 * shapes the gateway normalises: bare, `0`-prefixed, `91`/`+91`-prefixed,
 * with spaces, dashes, or parens anywhere.
 */
export function tenDigitMobile(raw: string): string | null {
  const digits = raw.replace(/\D/g, "");
  if (digits.length === 10) return digits;
  if (digits.length === 11 && digits.startsWith("0")) return digits.slice(1);
  if (digits.length === 12 && digits.startsWith("91")) return digits.slice(2);
  return null;
}

export function validateFullName(value: string): string | null {
  const trimmed = value.trim();
  if (!trimmed) return "Full name is required.";
  if (trimmed.length < 2 || trimmed.length > 120) return "Must be 2-120 characters.";
  return null;
}

export function validateRollNumber(value: string): string | null {
  const trimmed = value.trim();
  if (!trimmed) return "Roll number is required.";
  if (!ROLL_NUMBER_RE.test(trimmed))
    return "Use the full roll number printed on your records — e.g. 25XHBML11450.";
  return null;
}

export function validatePhone(value: string): string | null {
  if (!value.trim()) return "Phone number is required.";
  const ten = tenDigitMobile(value);
  if (ten === null || !INDIAN_MOBILE_RE.test(ten))
    return "Must be a valid 10-digit Indian mobile number.";
  return null;
}

export function validateEmail(value: string): string | null {
  const trimmed = value.trim();
  if (!trimmed) return "Email is required.";
  if (!EMAIL_RE.test(trimmed)) return "Must be a valid email address.";
  return null;
}

export function validatePassword(value: string): string | null {
  if (!value) return "Password is required.";
  if (value.length < 8) return "Must be at least 8 characters.";
  if (value.length > 128) return "Must be 128 characters or fewer.";
  return null;
}

export function validateConfirmPassword(password: string, confirm: string): string | null {
  if (!confirm) return "Please confirm your password.";
  if (password !== confirm) return "Passwords do not match.";
  return null;
}

export function validateRequiredSelection(value: string, label: string): string | null {
  return value ? null : `Please select your ${label}.`;
}

/** Drop the `null`s so a form can test `Object.keys(errors).length === 0`. */
export function compact(errors: Record<string, string | null>): FieldErrors {
  const out: FieldErrors = {};
  for (const [field, message] of Object.entries(errors)) {
    if (message) out[field] = message;
  }
  return out;
}
