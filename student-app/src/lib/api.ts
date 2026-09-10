import axios from 'axios';

/**
 * Central axios instance for the Student app.
 * - baseURL /api  (proxied by Vite to backend in dev)
 * - Attaches Authorization header from stored tokens
 * - Handles 401 by attempting refresh, then redirecting to /login
 *
 * Token storage respects "Remember Me":
 *   remembered  -> localStorage  (survives browser restart, ~30 days server-side)
 *   not remembered -> sessionStorage (cleared on tab close, ~8h server-side)
 */

const TOKEN_KEY = 'dg_access_token';
const REFRESH_KEY = 'dg_refresh_token';

function getToken(): string | null {
  return localStorage.getItem(TOKEN_KEY) ?? sessionStorage.getItem(TOKEN_KEY);
}

function getRefreshToken(): string | null {
  return localStorage.getItem(REFRESH_KEY) ?? sessionStorage.getItem(REFRESH_KEY);
}

function getStorageForCurrentSession(): Storage {
  // If token lives in localStorage, user chose Remember Me.
  if (localStorage.getItem(TOKEN_KEY) || localStorage.getItem(REFRESH_KEY)) return localStorage;
  return sessionStorage;
}

export function persistTokens(
  tokens: { accessToken: string; refreshToken: string },
  rememberMe: boolean,
) {
  // Clear both storages first to avoid stale duplicates
  localStorage.removeItem(TOKEN_KEY);
  localStorage.removeItem(REFRESH_KEY);
  sessionStorage.removeItem(TOKEN_KEY);
  sessionStorage.removeItem(REFRESH_KEY);

  const store = rememberMe ? localStorage : sessionStorage;
  store.setItem(TOKEN_KEY, tokens.accessToken);
  store.setItem(REFRESH_KEY, tokens.refreshToken);
}

export function clearTokens() {
  localStorage.removeItem(TOKEN_KEY);
  localStorage.removeItem(REFRESH_KEY);
  sessionStorage.removeItem(TOKEN_KEY);
  sessionStorage.removeItem(REFRESH_KEY);
  localStorage.removeItem('dg_user');
  sessionStorage.removeItem('dg_user');
}

export function getStoredAccessToken(): string | null {
  return getToken();
}

export const api = axios.create({
  baseURL: '/api',
  headers: { 'Content-Type': 'application/json' },
  timeout: 15000,
});

/**
 * Extract a user-facing message from an unknown thrown value.
 *
 * The backend replies with `{ success: false, error, details? }`; for
 * validation failures the first `details[].message` is the most specific thing
 * to show. Falls back to the axios message, then a caller-supplied default.
 */
export function getApiErrorMessage(error: unknown, fallback = 'Something went wrong. Please try again.'): string {
  const data = (error as { response?: { data?: unknown } })?.response?.data as
    | { error?: string; message?: string; details?: Array<{ message?: string }> }
    | undefined;

  const detail = data?.details?.find((d) => d?.message)?.message;
  if (detail) return detail;

  return data?.error ?? data?.message ?? (error as Error)?.message ?? fallback;
}

api.interceptors.request.use((config) => {
  const token = getToken();
  if (token) config.headers.Authorization = `Bearer ${token}`;
  return config;
});

let isRefreshing = false;
let queue: Array<{ resolve: (v: string | null) => void; reject: (e: unknown) => void }> = [];

function flushQueue(token: string | null, error: unknown) {
  queue.forEach(({ resolve, reject }) => (error ? reject(error) : resolve(token)));
  queue = [];
}

api.interceptors.response.use(
  (res) => res,
  async (error) => {
    const original = error.config as (typeof error.config & { _retried?: boolean }) | undefined;
    const status = error.response?.status;

    if (status === 401 && original && !original._retried) {
      if (isRefreshing) {
        return new Promise<string | null>((resolve, reject) => {
          queue.push({ resolve, reject });
        }).then((token) => {
          if (token && original.headers) {
            (original.headers as Record<string, string>).Authorization = `Bearer ${token}`;
          }
          return api(original);
        });
      }

      original._retried = true;
      isRefreshing = true;

      const refreshToken = getRefreshToken();
      if (!refreshToken) {
        isRefreshing = false;
        clearTokens();
        window.location.href = '/login';
        return Promise.reject(error);
      }

      try {
        // Attempt refresh — backend endpoint per spec: POST /api/auth/refresh
        const { data } = await axios.post('/api/auth/refresh', { refreshToken });
        const newAccess: string = data.accessToken ?? data.access_token ?? data.token;
        const newRefresh: string = data.refreshToken ?? data.refresh_token ?? refreshToken;

        // Preserve original rememberMe choice
        const storage = getStorageForCurrentSession();
        storage.setItem(TOKEN_KEY, newAccess);
        storage.setItem(REFRESH_KEY, newRefresh);

        flushQueue(newAccess, null);

        if (original.headers) {
          (original.headers as Record<string, string>).Authorization = `Bearer ${newAccess}`;
        }
        return api(original);
      } catch (refreshError) {
        flushQueue(null, refreshError);
        clearTokens();
        window.location.href = '/login';
        return Promise.reject(refreshError);
      } finally {
        isRefreshing = false;
      }
    }

    return Promise.reject(error);
  },
);

export default api;
