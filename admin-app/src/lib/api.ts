/**
 * Digi Guru Admin — API client
 * - Base URL proxied via Vite to backend (http://localhost:4000)
 * - Attaches admin JWT from localStorage on every request
 * - 401 → clears session + redirects to /admin/login
 * - 403 → surfaces "Access Denied" so UI can render the 403 page
 */

import axios, { AxiosError, InternalAxiosRequestConfig } from 'axios';

export const ADMIN_TOKEN_KEY = 'admin_token';
export const ADMIN_USER_KEY = 'admin_user';

export interface ApiErrorBody {
  success: false;
  error: string;
  code?: string;
}

export interface ApiSuccessBody<T> {
  success: true;
  data: T;
  meta?: { total: number; page: number; limit: number };
}

export type ApiResponse<T> = ApiSuccessBody<T> | ApiErrorBody;

// ── Axios instance ──────────────────────────────────────────────────────────

export const api = axios.create({
  baseURL: '/api',
  timeout: 15000,
  headers: { 'Content-Type': 'application/json' },
  withCredentials: true, // allow refresh cookie if backend uses httpOnly cookie
});

// ── Request interceptor — attach token ─────────────────────────────────────

api.interceptors.request.use((config: InternalAxiosRequestConfig) => {
  const token = localStorage.getItem(ADMIN_TOKEN_KEY);
  if (token && config.headers) {
    config.headers.Authorization = `Bearer ${token}`;
  }
  return config;
});

// ── Response interceptor — handle auth errors ───────────────────────────────

let isRedirecting = false;

function redirectToLogin(): void {
  if (isRedirecting) return;
  isRedirecting = true;
  localStorage.removeItem(ADMIN_TOKEN_KEY);
  localStorage.removeItem(ADMIN_USER_KEY);
  // Use hard navigation so ProtectedAdminRoute re-evaluates cleanly.
  window.location.href = '/admin/login';
}

api.interceptors.response.use(
  (response) => response,
  (error: AxiosError<ApiErrorBody>) => {
    const status = error.response?.status;

    if (status === 401) {
      // Session expired or token invalid — force re-login.
      // Do NOT redirect if we are already on the login page (avoid loop).
      if (!window.location.pathname.startsWith('/admin/login')) {
        redirectToLogin();
      }
    }

    // 403 is intentionally NOT auto-redirected — callers render Access Denied.
    return Promise.reject(error);
  },
);

// ── Typed helpers ───────────────────────────────────────────────────────────

export function getErrorMessage(error: unknown): string {
  if (axios.isAxiosError<ApiErrorBody>(error)) {
    if (error.response?.data?.error) return error.response.data.error;
    if (error.response?.status === 403) return 'Access Denied — Admin only.';
    if (error.message) return error.message;
  }
  if (error instanceof Error) return error.message;
  return 'An unexpected error occurred. Please try again.';
}

export function isForbiddenError(error: unknown): boolean {
  return axios.isAxiosError(error) && error.response?.status === 403;
}

// ── Auth helpers ────────────────────────────────────────────────────────────

export interface AdminUser {
  id: string;
  email: string;
  fullName: string;
  role: 'ADMIN';
}

export function getStoredAdminUser(): AdminUser | null {
  try {
    const raw = localStorage.getItem(ADMIN_USER_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as AdminUser;
    if (parsed.role !== 'ADMIN') return null;
    return parsed;
  } catch {
    return null;
  }
}

export function getStoredToken(): string | null {
  return localStorage.getItem(ADMIN_TOKEN_KEY);
}

export function clearAdminSession(): void {
  localStorage.removeItem(ADMIN_TOKEN_KEY);
  localStorage.removeItem(ADMIN_USER_KEY);
}

export function persistAdminSession(token: string, user: AdminUser): void {
  localStorage.setItem(ADMIN_TOKEN_KEY, token);
  localStorage.setItem(ADMIN_USER_KEY, JSON.stringify(user));
}
