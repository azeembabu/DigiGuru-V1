import { NavLink } from 'react-router-dom';
import { LayoutDashboard, BookOpen, TrendingUp, FileText, ClipboardList } from 'lucide-react';

/**
 * Left sidebar navigation (desktop ≥md): logo, nav items, coming-soon items.
 * Hidden on mobile — MobileNavigation handles bottom nav there.
 */

const READY_ITEMS = [
  { to: '/student/dashboard', label: 'Dashboard', icon: LayoutDashboard },
  { to: '/student/courses', label: 'Courses', icon: BookOpen },
  { to: '/student/progress', label: 'Progress', icon: TrendingUp },
] as const;

const COMING_SOON_ITEMS = [
  { label: 'Notes', icon: FileText },
  { label: 'Tests', icon: ClipboardList },
] as const;

const ACTIVE = 'flex items-center gap-2.5 rounded-lg px-3 py-2.5 text-sm font-semibold text-dg-900 bg-dg-50';
const IDLE = 'flex items-center gap-2.5 rounded-lg px-3 py-2.5 text-sm font-medium text-ink-500 hover:text-ink-900 hover:bg-slate-50 transition-colors';
const COMING_LABEL = 'block text-[11px] font-normal text-slate-400';

export function Sidebar() {
  return (
    <aside className="hidden w-56 shrink-0 flex-col border-r border-slate-200 bg-white md:flex">
      {/* Logo */}
      <div className="flex h-14 items-center gap-2.5 px-5">
        <span
          className="flex h-7 w-7 items-center justify-center rounded-lg bg-dg-900 text-white text-[13px] font-bold"
          aria-hidden
        >
          ◆
        </span>
        <span className="text-sm font-semibold tracking-tight text-ink-900">Digi Guru</span>
      </div>

      {/* Nav */}
      <nav className="flex-1 px-3 py-2 space-y-1" aria-label="Sidebar">
        {READY_ITEMS.map(({ to, label, icon: Icon }) => (
          <NavLink
            key={to}
            to={to}
            className={({ isActive }) => [isActive ? ACTIVE : IDLE].join(' ')}
          >
            <Icon size={16} aria-hidden />
            {label}
          </NavLink>
        ))}

        <div className="mt-3 pt-3 border-t border-slate-100" />

        {COMING_SOON_ITEMS.map(({ label, icon: Icon }) => (
          <div
            key={label}
            aria-disabled
            className="flex items-center gap-2.5 rounded-lg px-3 py-2.5 text-sm text-slate-400"
          >
            <Icon size={16} aria-hidden />
            <span>
              {label}
              <span className={COMING_LABEL}>Coming soon</span>
            </span>
          </div>
        ))}
      </nav>
    </aside>
  );
}
