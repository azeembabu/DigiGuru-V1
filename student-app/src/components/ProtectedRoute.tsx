import * as React from 'react';
import { Navigate, useLocation } from 'react-router-dom';
import { useAuthStore } from '../store/authStore';

export function ProtectedRoute({ children }: { children: React.ReactNode }) {
  const { isAuthenticated, isHydrated } = useAuthStore();
  const location = useLocation();

  if (!isHydrated) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-nature">
        <div className="flex items-center gap-3 rounded-full bg-white/90 px-6 py-3 shadow-glass backdrop-blur">
          <span className="h-5 w-5 animate-spin rounded-full border-2 border-dg-600 border-t-transparent" aria-hidden />
          <span className="text-sm font-medium text-ink-700">Loading…</span>
        </div>
      </div>
    );
  }

  if (!isAuthenticated) {
    return <Navigate to="/login" replace state={{ from: location }} />;
  }

  return <>{children}</>;
}
