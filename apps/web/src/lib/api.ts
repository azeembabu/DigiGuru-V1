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

/**
 * Endpoints that must never trigger a refresh-and-retry.
 *
 * A 401 from any of these is the answer, not a stale token — retrying
 * `/auth/refresh` after it 401s would recurse, and retrying a failed login
 * would double-count against the rate limiter on `/auth/*`.
 */
const NO_REFRESH = [
  "/auth/refresh",
  "/auth/login",
  "/auth/signup",
  "/auth/logout",
  "/auth/forgot-password",
  "/auth/reset-password",
];

/**
 * In-flight refresh, shared by every caller.
 *
 * A screen typically fires several requests at once, so without this a single
 * expiry would send N parallel refreshes. That is not just wasteful: the
 * gateway ROTATES the refresh token and revokes the presented one
 * (`apps/gateway/src/auth/refresh.rs`), so the second request would arrive
 * with a token the first had already revoked and be treated as a replay.
 */
let refreshInFlight: Promise<boolean> | null = null;

function refreshAccessToken(): Promise<boolean> {
  refreshInFlight ??= (async () => {
    try {
      const response = await fetch(`${API_BASE_URL}/auth/refresh`, {
        method: "POST",
        credentials: "include",
      });
      return response.ok;
    } catch {
      return false;
    } finally {
      // Cleared on the microtask after resolution so concurrent callers that
      // are already awaiting share this result, while the next expiry starts
      // a fresh attempt.
      queueMicrotask(() => {
        refreshInFlight = null;
      });
    }
  })();

  return refreshInFlight;
}

/**
 * Send the request, and if it comes back 401, silently rotate the session and
 * send it once more.
 *
 * The access JWT lives 15 minutes (`.claude/rules/api-conventions.md`) while
 * the refresh token lives days, so without this every screen breaks a quarter
 * of an hour into a session and the admin is told "Authentication is
 * required" for work they are in the middle of. Both tokens are httpOnly
 * cookies, so the browser cannot check expiry in advance — a 401 is the only
 * signal available, which makes retry-on-401 the mechanism rather than a
 * timer.
 */
async function apiFetchWithRetry(path: string, init?: RequestInit): Promise<Response> {
  const response = await apiFetchRaw(path, init);

  if (response.status !== 401 || NO_REFRESH.some((prefix) => path.startsWith(prefix))) {
    return response;
  }

  // A retried body must be re-readable. A `File`/`Blob` (the PDF upload) is;
  // a stream is not, so those are left to fail rather than silently sending a
  // truncated body.
  if (init?.body instanceof ReadableStream) return response;

  const refreshed = await refreshAccessToken();
  if (!refreshed) {
    // The session is genuinely over, not merely stale. Send them to sign in
    // rather than leaving a half-usable screen telling them "Authentication
    // is required" with no way to act on it. `next` brings them back.
    if (typeof window !== "undefined" && !window.location.pathname.startsWith("/login")) {
      const next = window.location.pathname + window.location.search;
      // A hard navigation, deliberately, not `router.push`: this module is
      // not a component and has no router, and a full document load is what
      // we want anyway — it discards every screen's cached state along with
      // the dead session, so nothing survives to render stale admin data.
      // eslint-disable-next-line @next/next/no-location-assign-relative-destination
      window.location.assign(`/login?next=${encodeURIComponent(next)}`);
    }
    return response;
  }

  return apiFetchRaw(path, init);
}

export async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await apiFetchWithRetry(path, init);
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
  const response = await apiFetchWithRetry(path, init);
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
