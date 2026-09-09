import * as React from 'react';
import { Link, NavLink, useNavigate } from 'react-router-dom';
import { useAuthStore } from '../store/authStore';
import { clearTokens } from '../lib/api';

function Logo() {
  return (
    <Link to="/student/dashboard" className="flex items-center gap-2.5 focus-ring rounded-xl">
      <span
        className="flex h-8 w-8 items-center justify-center rounded-xl bg-dg-900 text-white text-[15px] font-bold leading-none shadow-sm"
        aria-hidden
      >
        ◆
      </span>
      <span className="text-[17px] font-semibold tracking-tight text-ink-900">
        Digi Guru
      </span>
    </Link>
  );
}

export function Layout({ children }: { children: React.ReactNode }) {
  const { isAuthenticated, user, logout } = useAuthStore();
  const navigate = useNavigate();
  const [mobileOpen, setMobileOpen] = React.useState(false);

  function handleLogout() {
    clearTokens();
    logout();
    navigate('/login', { replace: true });
  }

  return (
    <div className="min-h-screen bg-[#F8FAF8]">
      <header className="sticky top-0 z-30 border-b border-slate-200/70 bg-white/85 backdrop-blur-xl">
        <div className="mx-auto flex h-[64px] max-w-6xl items-center justify-between gap-4 px-4 sm:px-6 lg:px-8">
          <Logo />

          {/* Desktop nav */}
          <nav className="hidden items-center gap-1 sm:flex" aria-label="Primary">
            {isAuthenticated ? (
              <>
                <NavLink
                  to="/student/dashboard"
                  className={({ isActive }) =>
                    [
                      'rounded-full px-4 py-2 text-sm font-medium transition-colors',
                      isActive ? 'bg-dg-900 text-white' : 'text-ink-700 hover:bg-slate-100 hover:text-ink-900',
                    ].join(' ')
                  }
                >
                  Dashboard
                </NavLink>
                <NavLink
                  to="/student/profile"
                  className={({ isActive }) =>
                    [
                      'rounded-full px-4 py-2 text-sm font-medium transition-colors',
                      isActive ? 'bg-dg-900 text-white' : 'text-ink-700 hover:bg-slate-100 hover:text-ink-900',
                    ].join(' ')
                  }
                >
                  Profile
                </NavLink>
                <NavLink
                  to="/student/courses"
                  className={({ isActive }) =>
                    [
                      'rounded-full px-4 py-2 text-sm font-medium transition-colors',
                      isActive ? 'bg-dg-900 text-white' : 'text-ink-700 hover:bg-slate-100 hover:text-ink-900',
                    ].join(' ')
                  }
                >
                  Courses
                </NavLink>
                <NavLink
                  to="/student/settings"
                  className={({ isActive }) =>
                    [
                      'rounded-full px-4 py-2 text-sm font-medium transition-colors',
                      isActive ? 'bg-dg-900 text-white' : 'text-ink-700 hover:bg-slate-100 hover:text-ink-900',
                    ].join(' ')
                  }
                >
                  Settings
                </NavLink>
                <span className="mx-2 h-5 w-px bg-slate-200" aria-hidden />
                <span className="hidden text-sm text-ink-500 lg:inline">
                  {user?.fullName ?? user?.rollNumber ?? ''}
                </span>
                <button
                  onClick={handleLogout}
                  className="ml-2 rounded-full border border-slate-200 bg-white px-4 py-2 text-sm font-medium text-ink-700 transition hover:bg-slate-50 hover:text-ink-900 focus-ring"
                >
                  Logout
                </button>
              </>
            ) : (
              <>
                <Link
                  to="/login"
                  className="rounded-full px-4 py-2 text-sm font-medium text-ink-700 hover:bg-slate-100 hover:text-ink-900"
                >
                  Log in
                </Link>
                <Link
                  to="/signup"
                  className="rounded-full bg-dg-900 px-5 py-2.5 text-sm font-medium text-white shadow-sm transition hover:bg-dg-950 focus-ring"
                >
                  Get Started
                </Link>
              </>
            )}
          </nav>

          {/* Mobile toggle */}
          <button
            type="button"
            onClick={() => setMobileOpen((v) => !v)}
            className="inline-flex h-9 w-9 items-center justify-center rounded-xl border border-slate-200 bg-white text-ink-700 shadow-sm sm:hidden focus-ring"
            aria-expanded={mobileOpen}
            aria-label="Toggle menu"
          >
            <svg width="18" height="18" viewBox="0 0 18 18" fill="none" aria-hidden>
              <path d="M3 5h12M3 9h12M3 13h12" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
            </svg>
          </button>
        </div>

        {mobileOpen && (
          <div className="border-t border-slate-200 bg-white px-4 py-4 sm:hidden">
            {isAuthenticated ? (
              <div className="flex flex-col gap-2">
                <NavLink
                  to="/student/dashboard"
                  onClick={() => setMobileOpen(false)}
                  className={({ isActive }) =>
                    [
                      'rounded-xl px-4 py-3 text-sm font-medium',
                      isActive ? 'bg-dg-900 text-white' : 'bg-slate-50 text-ink-700',
                    ].join(' ')
                  }
                >
                  Dashboard
                </NavLink>
                <NavLink
                  to="/student/profile"
                  onClick={() => setMobileOpen(false)}
                  className={({ isActive }) =>
                    [
                      'rounded-xl px-4 py-3 text-sm font-medium',
                      isActive ? 'bg-dg-900 text-white' : 'bg-slate-50 text-ink-700',
                    ].join(' ')
                  }
                >
                  Profile
                </NavLink>
                <NavLink
                  to="/student/courses"
                  onClick={() => setMobileOpen(false)}
                  className={({ isActive }) =>
                    [
                      'rounded-xl px-4 py-3 text-sm font-medium',
                      isActive ? 'bg-dg-900 text-white' : 'bg-slate-50 text-ink-700',
                    ].join(' ')
                  }
                >
                  Courses
                </NavLink>
                <NavLink
                  to="/student/settings"
                  onClick={() => setMobileOpen(false)}
                  className={({ isActive }) =>
                    [
                      'rounded-xl px-4 py-3 text-sm font-medium',
                      isActive ? 'bg-dg-900 text-white' : 'bg-slate-50 text-ink-700',
                    ].join(' ')
                  }
                >
                  Settings
                </NavLink>
                <button
                  onClick={handleLogout}
                  className="rounded-xl border border-slate-200 bg-white px-4 py-3 text-left text-sm font-medium text-ink-700"
                >
                  Logout
                </button>
              </div>
            ) : (
              <div className="flex flex-col gap-2">
                <Link
                  to="/login"
                  onClick={() => setMobileOpen(false)}
                  className="rounded-xl bg-slate-50 px-4 py-3 text-center text-sm font-medium text-ink-700"
                >
                  Log in
                </Link>
                <Link
                  to="/signup"
                  onClick={() => setMobileOpen(false)}
                  className="rounded-full bg-dg-900 px-4 py-3 text-center text-sm font-medium text-white"
                >
                  Get Started
                </Link>
              </div>
            )}
          </div>
        )}
      </header>

      <main>{children}</main>

      <footer className="border-t border-slate-200/60 bg-white/60 py-8">
        <div className="mx-auto max-w-6xl px-4 sm:px-6 lg:px-8">
          <p className="text-center text-sm text-ink-500">
            © {new Date().getFullYear()} Digi Guru — Learn with your personal AI teacher.
          </p>
        </div>
      </footer>
    </div>
  );
}
