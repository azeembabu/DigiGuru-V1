import * as React from 'react';
import { NavLink } from 'react-router-dom';
import { Home, BookOpen, TrendingUp, User } from 'lucide-react';

/**
 * Mobile bottom navigation (spec §16): fixed bar with Home/Courses/Progress/Profile.
 * Desktop: hidden (md:flex header nav takes over).
 */

const ITEMS = [
  { to: '/student/dashboard', label: 'Home', icon: Home },
  { to: '/student/courses', label: 'Courses', icon: BookOpen },
  { to: '/student/progress', label: 'Progress', icon: TrendingUp },
  { to: '/student/profile', label: 'Profile', icon: User },
] as const;

export function MobileNavigation() {
  return (
    <nav
      className="fixed inset-x-0 bottom-0 z-30 border-t border-slate-200 bg-white md:hidden"
      aria-label="Mobile"
    >
      <div className="grid grid-cols-4">
        {ITEMS.map(({ to, label, icon: Icon }) => (
          <NavLink
            key={to}
            to={to}
            className={({ isActive }) =>
              [
                'flex flex-col items-center gap-1 py-2.5 text-[11px] font-medium transition-colors',
                isActive ? 'text-dg-900' : 'text-ink-400',
              ].join(' ')
            }
          >
            <Icon size={20} aria-hidden />
            {label}
          </NavLink>
        ))}
      </div>
    </nav>
  );
}
