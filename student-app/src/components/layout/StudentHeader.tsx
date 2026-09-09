import * as React from 'react';
import { Link, NavLink, useNavigate } from 'react-router-dom';
import { LayoutDashboard, BookOpen, TrendingUp, ChevronDown, LogOut, Settings, User } from 'lucide-react';
import { useAuthStore } from '../../store/authStore';
import { api, clearTokens } from '../../lib/api';

/**
 * Sticky desktop header (spec §3/§4): white, 64–72px, bottom border, minimal shadow.
 * Nav: Dashboard / Courses / Progress. Right: avatar dropdown (Profile/Settings/Logout).
 * No Admin option anywhere (spec §3).
 */

const NAV_ITEMS = [
  { to: '/student/dashboard', label: 'Dashboard', icon: LayoutDashboard },
  { to: '/student/courses', label: 'Courses', icon: BookOpen },
  { to: '/student/progress', label: 'Progress', icon: TrendingUp },
] as const;

const NAV_ACTIVE = 'text-dg-900 font-semibold';
const NAV_IDLE = 'text-ink-500 hover:text-ink-900 hover:bg-slate-100';

export function StudentHeader() {
  const user = useAuthStore((s) => s.user);
  const logout = useAuthStore((s) => s.logout);
  const navigate = useNavigate();
  const [menuOpen, setMenuOpen] = React.useState(false);
  const menuRef = React.useRef<HTMLDivElement>(null);

  // Close dropdown on outside click
  React.useEffect(() => {
    function onClick(e: MouseEvent) {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) setMenuOpen(false);
    }
    document.addEventListener('mousedown', onClick);
    return () => document.removeEventListener('mousedown', onClick);
  }, []);

  const initials = (user?.fullName ?? '')
    .split(' ')
    .filter(Boolean)
    .slice(0, 2)
    .map((w) => w[0]?.toUpperCase())
    .join('') || 'S';

  async function handleLogout() {
    setMenuOpen(false);
    try {
      await api.post('/auth/logout');
    } catch {
      // Session may already be expired — clear local state regardless
    }
    clearTokens();
    logout();
    navigate('/login', { replace: true });
  }

  return (
    <header className="sticky top-0 z-30 border-b border-slate-200 bg-white">
      <div className="mx-auto flex h-16 max-w-6xl items-center justify-between gap-4 px-4 sm:px-6 lg:px-8">
        {/* Logo */}
        <Link to="/student/dashboard" className="flex items-center gap-2.5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-dg-600 rounded-lg">
          <span className="flex h-8 w-8 items-center justify-center rounded-lg bg-dg-900 text-white text-[15px] font-bold" aria-hidden>
            ◆
          </span>
          <span className="text-lg font-semibold tracking-tight text-ink-900">Digi Guru</span>
        </Link>

        {/* Desktop nav */}
        <nav className="hidden items-center gap-1 md:flex" aria-label="Primary">
          {NAV_ITEMS.map(({ to, label, icon: Icon }) => (
            <NavLink
              key={to}
              to={to}
              className={({ isActive }) =>
                [
                  'flex items-center gap-2 rounded-lg px-3 py-2 text-sm font-medium transition-colors',
                  isActive ? NAV_ACTIVE : NAV_IDLE,
                ].join(' ')
              }
            >
              <Icon size={16} aria-hidden />
              {label}
            </NavLink>
          ))}
          {/* Coming-soon items — no fake navigation */}
          <span className="ml-1 flex items-center gap-2 rounded-lg px-3 py-2 text-sm text-slate-400" aria-disabled>
            Notes
          </span>
          <span className="flex items-center gap-2 rounded-lg px-3 py-2 text-sm text-slate-400" aria-disabled>
            Tests
          </span>
        </nav>

        {/* Avatar + dropdown */}
        <div className="relative" ref={menuRef}>
          <button
            type="button"
            onClick={() => setMenuOpen((v) => !v)}
            aria-expanded={menuOpen}
            aria-haspopup="menu"
            aria-label="Account menu"
            className="flex items-center gap-2 rounded-lg p-1 pr-1.5 transition-colors hover:bg-slate-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-dg-600"
          >
            <span className="flex h-9 w-9 items-center justify-center rounded-full bg-dg-900 text-sm font-semibold text-white">
              {initials}
            </span>
            <ChevronDown size={16} className="text-ink-500" aria-hidden />
          </button>

          {menuOpen && (
            <div
              role="menu"
              className="absolute right-0 mt-2 w-52 overflow-hidden rounded-xl border border-slate-200 bg-white py-1 shadow-lg"
            >
              <div className="border-b border-slate-100 px-4 py-3">
                <p className="truncate text-sm font-semibold text-ink-900">{user?.fullName ?? 'Student'}</p>
                <p className="mt-0.5 truncate text-xs text-ink-500">{user?.email ?? ''}</p>
              </div>
              <Link
                to="/student/profile"
                role="menuitem"
                onClick={() => setMenuOpen(false)}
                className="flex items-center gap-2.5 px-4 py-2.5 text-sm text-ink-700 hover:bg-slate-50 hover:text-ink-900"
              >
                <User size={16} aria-hidden /> Profile
              </Link>
              <Link
                to="/student/settings"
                role="menuitem"
                onClick={() => setMenuOpen(false)}
                className="flex items-center gap-2.5 px-4 py-2.5 text-sm text-ink-700 hover:bg-slate-50 hover:text-ink-900"
              >
                <Settings size={16} aria-hidden /> Settings
              </Link>
              <button
                type="button"
                role="menuitem"
                onClick={handleLogout}
                className="flex w-full items-center gap-2.5 px-4 py-2.5 text-left text-sm text-red-600 hover:bg-red-50"
              >
                <LogOut size={16} aria-hidden /> Logout
              </button>
            </div>
          )}
        </div>
      </div>
    </header>
  );
}
