/**
 * ProtectedAdminRoute
 * - If no token → redirect to /admin/login
 * - If token present but role is not ADMIN (e.g. a STUDENT JWT was pasted
 *   into localStorage) → show 403 Access Denied page
 * - Otherwise render children (the AdminLayout / protected pages)
 */

import * as React from 'react';
import { Navigate, useLocation } from 'react-router-dom';
import { useAuthStore } from '@/store/authStore';

export function ProtectedAdminRoute({ children }: { children: React.ReactNode }) {
  const { isAuthenticated, user } = useAuthStore();
  const location = useLocation();

  if (!isAuthenticated || !user) {
    return <Navigate to="/admin/login" state={{ from: location }} replace />;
  }

  if (user.role !== 'ADMIN') {
    return <AccessDenied />;
  }

  return <>{children}</>;
}

export function AccessDenied() {
  return (
    <div className="flex min-h-[60vh] flex-col items-center justify-center px-6 py-16 text-center">
      <div className="flex h-16 w-16 items-center justify-center rounded-2xl bg-red-50 text-red-600 ring-1 ring-red-200">
        <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
          <circle cx="12" cy="12" r="10" />
          <line x1="12" y1="8" x2="12" y2="12" />
          <line x1="12" y1="16" x2="12.01" y2="16" />
        </svg>
      </div>
      <h1 className="mt-6 text-2xl font-bold tracking-tight text-admin-900">Access Denied</h1>
      <p className="mt-2 max-w-md text-sm leading-6 text-admin-500">
        This area is restricted to administrators. Your current session does not have the required permissions.
        If you are a student, please use the Student portal. If you believe this is an error, contact support.
      </p>
      <p className="mt-2 font-mono text-xs text-admin-400">403 — Admin only</p>
      <div className="mt-6 flex gap-3">
        <a
          href="/admin/login"
          className="inline-flex h-9 items-center justify-center rounded-md bg-indigo-600 px-4 text-sm font-medium text-white shadow-sm hover:bg-indigo-700"
        >
          Go to Admin Login
        </a>
        <a
          href="/"
          className="inline-flex h-9 items-center justify-center rounded-md border border-admin-200 bg-white px-4 text-sm font-medium text-admin-700 hover:bg-admin-50"
        >
          Home
        </a>
      </div>
    </div>
  );
}
