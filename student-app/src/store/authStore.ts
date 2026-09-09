import { create } from 'zustand';

export type StudentUser = {
  id: string;
  fullName: string;
  rollNumber: string;
  email: string;
  phoneNumber: string;
  program: string;
  semester: string;
  lsc: string;
  role: 'STUDENT';
};

type AuthState = {
  user: StudentUser | null;
  isAuthenticated: boolean;
  isHydrated: boolean;
  setUser: (user: StudentUser | null) => void;
  setAuthenticated: (v: boolean) => void;
  hydrate: () => void;
  logout: () => void;
};

const USER_KEY = 'dg_user';

function readUserFromStorage(): StudentUser | null {
  const raw = localStorage.getItem(USER_KEY) ?? sessionStorage.getItem(USER_KEY);
  if (!raw) return null;
  try {
    return JSON.parse(raw) as StudentUser;
  } catch {
    return null;
  }
}

export const useAuthStore = create<AuthState>((set) => ({
  user: null,
  isAuthenticated: false,
  isHydrated: false,

  setUser: (user) => {
    set({ user, isAuthenticated: !!user });
    if (user) {
      // Persist in whichever storage already holds the token; default to localStorage
      const hasLocalToken = !!localStorage.getItem('dg_access_token');
      const store = hasLocalToken ? localStorage : sessionStorage;
      // Clear both first
      localStorage.removeItem(USER_KEY);
      sessionStorage.removeItem(USER_KEY);
      store.setItem(USER_KEY, JSON.stringify(user));
    } else {
      localStorage.removeItem(USER_KEY);
      sessionStorage.removeItem(USER_KEY);
    }
  },

  setAuthenticated: (v) => set({ isAuthenticated: v }),

  hydrate: () => {
    const user = readUserFromStorage();
    const token = localStorage.getItem('dg_access_token') ?? sessionStorage.getItem('dg_access_token');
    set({
      user,
      isAuthenticated: !!user && !!token,
      isHydrated: true,
    });
  },

  logout: () => {
    localStorage.removeItem(USER_KEY);
    sessionStorage.removeItem(USER_KEY);
    localStorage.removeItem('dg_access_token');
    localStorage.removeItem('dg_refresh_token');
    sessionStorage.removeItem('dg_access_token');
    sessionStorage.removeItem('dg_refresh_token');
    set({ user: null, isAuthenticated: false });
  },
}));
