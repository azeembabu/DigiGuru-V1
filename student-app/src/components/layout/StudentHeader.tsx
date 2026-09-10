import * as React from 'react';
import { Link, NavLink, useNavigate } from 'react-router-dom';
import { Bell, ChevronDown, LogOut, Settings, User } from 'lucide-react';
import { useAuthStore } from '../../store/authStore';
import { api, clearTokens } from '../../lib/api';

/**
 * Slim top header (h-14): page title on left, notification bell + avatar dropdown on right.
 * Visible on all screen sizes; on desktop it sits to the right of the sidebar.
 */

const NOTIFICATION_BADGE = false; // placeholder — no real notifications yet

interface Props {
  title: string;
}

export function StudentHeader({ title }: Props) {
  const user = useAuthStore((s) => s.user);
  const logout = useAuthStore((s) => s.logout);
  const navigate = useNavigate();
  const [menuOpen, setMenuOpen] = React.useState(false);
  const menuRef = React.useRef<HTMLDivElement>(null);

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
    <header className="sticky top-0 z-30 h-14 border-b border-slate-200 bg-white">
      <div className="flex h-full items-center justify-between gap-4 px-4 sm:px-6 lg:px-8">
        {/* Page title */}
        <h1 className="text-base font-semibold text-ink-900">{title}</h1>

        {/* Right side: bell + avatar */}
        <div className="flex items-center gap-2">
          {/* Notification bell — placeholder */}
          <button
            type="button"
            aria-label="Notifications"
            className="relative flex h-9 w-9 items-center justify-center rounded-lg text-ink-500 transition-colors hover:bg-slate-100 hover:text-ink-700 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-dg-600"
          >
            <Bell size={18} aria-hidden />
            {NOTIFICATION_BADGE && (
              <span className="absolute right-2 top-2 h-1.5 w-1.5 rounded-full bg-red-500" aria-hidden />
            )}
          </button>

          {/* Avatar dropdown */}
          <div className="relative" ref={menuRef}>
            <button
              type="button"
              onClick={() => setMenuOpen((v) => !v)}
              aria-expanded={menuOpen}
              aria-haspopup="menu"
              aria-label="Account menu"
              className="flex items-center gap-2 rounded-lg p-1 pr-1.5 transition-colors hover:bg-slate-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-dg-600"
            >
              <span className="flex h-8 w-8 items-center justify-center rounded-full bg-dg-900 text-xs font-semibold text-white">
                {initials}
              </span>
              <ChevronDown size={14} className="hidden text-ink-500 sm:block" aria-hidden />
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
      </div>
    </header>
  );
}
