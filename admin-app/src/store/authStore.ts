/**
 * Admin auth store — Zustand
 * Single source of truth for admin session. Persisted via localStorage
 * through lib/api helpers (not zustand persist — keeps token handling explicit).
 */

import { create } from 'zustand';
import {
  ADMIN_TOKEN_KEY,
  ADMIN_USER_KEY,
  clearAdminSession,
  persistAdminSession,
  type AdminUser,
} from '@/lib/api';

interface AuthState {
  token: string | null;
  user: AdminUser | null;
  isAuthenticated: boolean;
  /** Hydrate from localStorage on app boot */
  hydrate: () => void;
  /** Call after successful POST /api/auth/admin/login */
  setSession: (token: string, user: AdminUser) => void;
  logout: () => void;
}

export const useAuthStore = create<AuthState>((set) => ({
  token: null,
  user: null,
  isAuthenticated: false,

  hydrate: () => {
    try {
      const token = localStorage.getItem(ADMIN_TOKEN_KEY);
      const raw = localStorage.getItem(ADMIN_USER_KEY);
      if (!token || !raw) {
        set({ token: null, user: null, isAuthenticated: false });
        return;
      }
      const user = JSON.parse(raw) as AdminUser;
      if (user.role !== 'ADMIN' || !token) {
        clearAdminSession();
        set({ token: null, user: null, isAuthenticated: false });
        return;
      }
      set({ token, user, isAuthenticated: true });
    } catch {
      clearAdminSession();
      set({ token: null, user: null, isAuthenticated: false });
    }
  },

  setSession: (token, user) => {
    persistAdminSession(token, user);
    set({ token, user, isAuthenticated: true });
  },

  logout: () => {
    clearAdminSession();
    set({ token: null, user: null, isAuthenticated: false });
  },
}));
