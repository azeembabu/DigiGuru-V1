// Thin client for the Rust gateway (`apps/gateway`).
//
// The gateway serialises every failure as the single envelope documented in
// `.claude/rules/api-conventions.md`:
//   { "error": { "code": "VALIDATION_ERROR", "message": "..." } }
// There is no per-field array on the wire — `PublicError::Validation` joins
// its `FieldError`s into one string as "field: message; field2: message"
// (`crates/core/src/error.rs`). `fieldErrors()` below re-splits that so a form
// can put each message back under the input it belongs to.

export const API_BASE_URL =
  process.env.NEXT_PUBLIC_API_BASE_URL ?? "http://localhost:8080/api/v1";

export class ApiError extends Error {
  readonly code: string;
  readonly status: number;

  constructor(code: string, message: string, status: number) {
    super(message);
    this.name = "ApiError";
    this.code = code;
    this.status = status;
  }

  /** True when the gateway rejected the payload rather than the request itself. */
  get isValidation(): boolean {
    return this.code === "VALIDATION_ERROR";
  }

  /**
   * Split a joined validation message back into `{ field: message }`.
   * Returns an empty object for any non-validation error, so callers can
   * always spread the result without checking first.
   */
  fieldErrors(): Record<string, string> {
    if (!this.isValidation) return {};

    const out: Record<string, string> = {};
    for (const part of this.message.split("; ")) {
      const idx = part.indexOf(": ");
      if (idx === -1) continue;
      const field = part.slice(0, idx).trim();
      const message = part.slice(idx + 2).trim();
      // Keep the first message per field; the server may emit several.
      if (field && message && !(field in out)) out[field] = message;
    }
    return out;
  }
}

function isErrorEnvelope(value: unknown): value is { error: { code: string; message: string } } {
  if (typeof value !== "object" || value === null || !("error" in value)) return false;
  const { error } = value as { error: unknown };
  return (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    "message" in error &&
    typeof (error as { code: unknown }).code === "string" &&
    typeof (error as { message: unknown }).message === "string"
  );
}

/**
 * The one place a request actually leaves the browser. Split out from
 * `apiFetch` so `apiFetchPage` can read response headers before the body is
 * consumed — both share identical credential, error-envelope, and
 * network-failure handling.
 */
async function apiFetchRaw(path: string, init?: RequestInit): Promise<Response> {
  let response: Response;
  try {
    response = await fetch(`${API_BASE_URL}${path}`, {
      // Auth is an httpOnly cookie (`.claude/rules/security.md`), so every
      // call must opt into sending it — `fetch` omits cross-origin cookies.
      credentials: "include",
      ...init,
      headers: {
        "Content-Type": "application/json",
        ...init?.headers,
      },
    });
  } catch {
    // Network-level failure: the gateway never answered, so there is no
    // envelope to read. Surface it as the same shape as everything else.
    throw new ApiError(
      "NETWORK_ERROR",
      "Could not reach the server. Check your connection and try again.",
      0,
    );
  }
  return response;
}

/** Parse the body, converting any non-2xx into the `ApiError` envelope. */
async function readBody(response: Response): Promise<unknown> {
  const body: unknown = await response.json().catch(() => null);

  if (!response.ok) {
    if (isErrorEnvelope(body)) {
      throw new ApiError(body.error.code, body.error.message, response.status);
    }
    throw new ApiError(
      "INTERNAL_ERROR",
      "Something went wrong. Please try again later.",
      response.status,
    );
  }

  return body;
}

export async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await apiFetchRaw(path, init);
  return (await readBody(response)) as T;
}

/**
 * Like `apiFetch`, but also surfaces the total-row count the gateway puts in
 * `X-Total-Count` on every paginated admin list
 * (`.claude/rules/api-conventions.md`). The body stays a plain array — the
 * count rides in the header precisely so the shipped array shapes did not have
 * to be wrapped in an envelope.
 *
 * `total` falls back to the page length when the header is absent or
 * unparseable, so a list still renders (just without a true page count)
 * against a gateway build that predates the header.
 */
export async function apiFetchPage<T>(
  path: string,
  init?: RequestInit,
): Promise<{ items: T[]; total: number }> {
  const response = await apiFetchRaw(path, init);
  const body = (await readBody(response)) as T[];
  const header = response.headers.get("X-Total-Count");
  const parsed = header === null ? Number.NaN : Number.parseInt(header, 10);

  return {
    items: body,
    total: Number.isFinite(parsed) && parsed >= 0 ? parsed : body.length,
  };
}

/** Build a query string, dropping empty/undefined values so a cleared filter
 *  disappears from the URL instead of being sent as `q=`. */
export function query(params: Record<string, string | number | undefined | null>): string {
  const search = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value === undefined || value === null || value === "") continue;
    search.set(key, String(value));
  }
  const out = search.toString();
  return out ? `?${out}` : "";
}
