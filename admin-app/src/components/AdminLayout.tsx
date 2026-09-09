/**
 * AdminLayout — dark sidebar + top bar + Outlet
 * Professional slate/indigo theme, intentionally distinct from the student's
 * soft emerald glassmorphism.
 */

import * as React from 'react';
import { NavLink, Outlet, useNavigate } from 'react-router-dom';
import { useAuthStore } from '@/store/authStore';

type NavItem = { label: string; to: string; icon: React.ReactNode };

function navItemClass(isActive: boolean): string {
  return [
    'flex items-center gap-3 rounded-lg px-3 py-2.5 text-sm font-medium transition-colors',
    isActive
      ? 'bg-white/10 text-white'
      : 'text-slate-300 hover:bg-white/[0.06] hover:text-white',
  ].join(' ');
}

const NAV_ITEMS: NavItem[] = [
  {
    label: 'Dashboard',
    to: '/admin/dashboard',
    icon: (
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden>
        <rect x="3" y="3" width="7" height="7" rx="1.5" />
        <rect x="14" y="3" width="7" height="7" rx="1.5" />
        <rect x="3" y="14" width="7" height="7" rx="1.5" />
        <rect x="14" y="14" width="7" height="7" rx="1.5" />
      </svg>
    ),
  },
  {
    label: 'Students',
    to: '/admin/students',
    icon: (
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden>
        <path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2" />
        <circle cx="9" cy="7" r="4" />
        <path d="M22 21v-2a4 4 0 0 0-3-3.87" />
        <path d="M16 3.13a4 4 0 0 1 0 7.75" />
      </svg>
    ),
  },
  {
    label: 'Programs',
    to: '/admin/programs',
    icon: (
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden>
        <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20" />
        <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z" />
      </svg>
    ),
  },
  {
    label: 'Semesters',
    to: '/admin/semesters',
    icon: (
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden>
        <rect x="3" y="4" width="18" height="18" rx="2" />
        <path d="M16 2v4M8 2v4M3 10h18" />
      </svg>
    ),
  },
  {
    label: 'LSC',
    to: '/admin/lsc',
    icon: (
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden>
        <path d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
        <polyline points="9 22 9 12 15 12 15 22" />
      </svg>
    ),
  },
  {
    label: 'Settings',
    to: '/admin/settings',
    icon: (
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden>
        <circle cx="12" cy="12" r="3" />
        <path d="M12 1v4M12 19v4M4.22 4.22l2.83 2.83M17.66 17.66l2.83 2.83M1 12h4M19 12h4M4.22 19.78l2.83-2.83M17.66 6.34l2.83-2.83" />
      </svg>
    ),
  },
];

export function AdminLayout() {
  const [sidebarOpen, setSidebarOpen] = React.useState(false);
  const { user, logout } = useAuthStore();
  const navigate = useNavigate();

  const handleLogout = React.useCallback(() => {
    logout();
    navigate('/admin/login', { replace: true });
  }, [logout, navigate]);

  return (
    <div className="min-h-screen bg-admin-50">
      {/* Mobile backdrop */}
      {sidebarOpen && (
        <button
          type="button"
          aria-label="Close navigation"
          className="fixed inset-0 z-30 bg-admin-900/40 backdrop-blur-sm lg:hidden"
          onClick={() => setSidebarOpen(false)}
        />
      )}

      {/* Sidebar — dark, professional */}
      <aside
        className={[
          'fixed inset-y-0 left-0 z-40 flex w-[272px] flex-col border-r border-white/10 bg-admin-900',
          'transition-transform duration-200 lg:translate-x-0',
          sidebarOpen ? 'translate-x-0' : '-translate-x-full',
        ].join(' ')}
        aria-label="Admin navigation"
      >
        {/* Brand */}
        <div className="flex h-16 shrink-0 items-center gap-3 border-b border-white/10 px-5">
          <div className="flex h-9 w-9 items-center justify-center rounded-lg bg-indigo-600 text-white shadow-sm">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden>
              <path d="M12 2L2 7l10 5 10-5-10-5z" />
              <path d="M2 17l10 5 10-5" />
              <path d="M2 12l10 5 10-5" />
            </svg>
          </div>
          <div className="min-w-0">
            <p className="text-sm font-semibold leading-none tracking-tight text-white">Digi Guru</p>
            <p className="mt-1 text-xs font-medium tracking-widest text-slate-400">ADMIN PORTAL</p>
          </div>
        </div>

        {/* Nav */}
        <nav className="flex-1 overflow-y-auto px-3 py-4 admin-scrollbar">
          <p className="mb-2 px-3 text-xs font-semibold uppercase tracking-widest text-slate-500">Manage</p>
          <ul className="space-y-1">
            {NAV_ITEMS.map((item) => (
              <li key={item.to}>
                <NavLink to={item.to} className={({ isActive }) => navItemClass(isActive)}>
                  <span className="shrink-0">{item.icon}</span>
                  <span>{item.label}</span>
                </NavLink>
              </li>
            ))}
          </ul>

          <div className="mt-6 rounded-lg border border-white/10 bg-white/[0.04] p-3">
            <p className="text-xs font-medium text-slate-300">Need help?</p>
            <p className="mt-1 text-xs leading-5 text-slate-400">
              Admin actions are audited. All student data access is logged.
            </p>
          </div>
        </nav>

        {/* Sidebar footer — user */}
        <div className="border-t border-white/10 p-3">
          <div className="flex items-center gap-3 rounded-lg bg-white/[0.06] px-3 py-2.5">
            <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-indigo-600 text-xs font-semibold text-white">
              {(user?.fullName ?? 'A').slice(0, 1).toUpperCase()}
            </div>
            <div className="min-w-0 flex-1">
              <p className="truncate text-sm font-medium text-white">{user?.fullName ?? 'Admin'}</p>
              <p className="truncate text-xs text-slate-400">{user?.email ?? ''}</p>
            </div>
            <button
              type="button"
              onClick={handleLogout}
              title="Log out"
              className="shrink-0 rounded-md p-1.5 text-slate-400 hover:bg-white/10 hover:text-white"
            >
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden>
                <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4" />
                <polyline points="16 17 21 12 16 7" />
                <line x1="21" y1="12" x2="9" y2="12" />
              </svg>
            </button>
          </div>
        </div>
      </aside>

      {/* Main column */}
      <div className="lg:pl-[272px]">
        {/* Top bar */}
        <header className="sticky top-0 z-20 flex h-16 items-center gap-4 border-b border-admin-200 bg-white/80 px-4 backdrop-blur supports-[backdrop-filter]:bg-white/60 lg:px-6">
          <button
            type="button"
            className="inline-flex h-9 w-9 items-center justify-center rounded-md border border-admin-200 bg-white text-admin-600 hover:bg-admin-50 lg:hidden"
            onClick={() => setSidebarOpen((v) => !v)}
            aria-label={sidebarOpen ? 'Close menu' : 'Open menu'}
            aria-expanded={sidebarOpen}
          >
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden>
              <line x1="3" y1="6" x2="21" y2="6" />
              <line x1="3" y1="12" x2="21" y2="12" />
              <line x1="3" y1="18" x2="21" y2="18" />
            </svg>
          </button>

          <div className="min-w-0 flex-1">
            <p className="hidden text-sm font-medium text-admin-900 lg:block">
              Administration
              <span className="font-normal text-admin-500"> — manage students, programs, and centres</span>
            </p>
          </div>

          <div className="flex items-center gap-2">
            <div className="hidden items-center gap-3 lg:flex">
              <div className="text-right">
                <p className="text-sm font-medium leading-none text-admin-900">{user?.fullName ?? 'Admin'}</p>
                <p className="mt-1 text-xs text-admin-500">{user?.email ?? ''}</p>
              </div>
              <div className="flex h-9 w-9 items-center justify-center rounded-full bg-admin-900 text-sm font-semibold text-white">
                {(user?.fullName ?? 'A').slice(0, 1).toUpperCase()}
              </div>
            </div>
            <button
              type="button"
              onClick={handleLogout}
              className="inline-flex h-9 items-center justify-center rounded-md border border-admin-200 bg-white px-3 text-sm font-medium text-admin-700 hover:bg-admin-50"
            >
              Logout
            </button>
          </div>
        </header>

        {/* Page content */}
        <main className="px-4 py-6 lg:px-6 lg:py-8">
          <Outlet />
        </main>

        <footer className="border-t border-admin-200 px-4 py-4 lg:px-6">
          <p className="text-center text-xs text-admin-400">
            Digi Guru Admin Portal — separate from the Student app. All actions are logged.
          </p>
        </footer>
      </div>
    </div>
  );
}
