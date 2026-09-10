import { useLocation } from 'react-router-dom';
import { StudentHeader } from './StudentHeader';
import { MobileNavigation } from './MobileNavigation';
import { Sidebar } from './Sidebar';

/**
 * Student app shell:
 * - Desktop (≥md): Sidebar (left) + slim header (top-right) + main content
 * - Mobile (<md): slim header (top) + main content + bottom mobile nav
 */

const ROUTE_TITLE: Record<string, string> = {
  '/student/dashboard': 'Dashboard',
  '/student/courses': 'Courses',
  '/student/progress': 'Progress',
  '/student/profile': 'Profile',
  '/student/settings': 'Settings',
};

function pageTitle(pathname: string): string {
  return ROUTE_TITLE[pathname] ?? 'Dashboard';
}

export function StudentLayout({ children }: { children: React.ReactNode }) {
  const { pathname } = useLocation();
  const title = pageTitle(pathname);

  return (
    <div className="min-h-screen bg-[#F8FAFC]">
      {/* Desktop: sidebar + header+content column */}
      <div className="hidden md:flex md:h-screen md:overflow-hidden">
        <Sidebar />
        <div className="flex flex-1 flex-col overflow-y-auto">
          <StudentHeader title={title} />
          <main className="flex-1 px-4 py-6 sm:px-6 lg:px-8">{children}</main>
        </div>
      </div>

      {/* Mobile: header + content + bottom nav */}
      <div className="md:hidden">
        <StudentHeader title={title} />
        <main className="mx-auto w-full max-w-6xl px-4 pb-24 pt-4 sm:px-6 lg:px-8">{children}</main>
      </div>

      <MobileNavigation />
    </div>
  );
}
