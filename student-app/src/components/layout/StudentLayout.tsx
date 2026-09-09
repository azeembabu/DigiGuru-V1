import * as React from 'react';
import { StudentHeader } from './StudentHeader';
import { MobileNavigation } from './MobileNavigation';

/**
 * Student app shell (spec §18): top navigation on desktop + bottom navigation
 * on mobile. Main content scrolls between them; bottom padding clears the
 * fixed mobile bar.
 */
export function StudentLayout({ children }: { children: React.ReactNode }) {
  return (
    <div className="min-h-screen bg-[#F8FAFC]">
      <StudentHeader />
      <main className="mx-auto w-full max-w-6xl px-4 pb-24 pt-6 sm:px-6 md:pb-10 lg:px-8">{children}</main>
      <MobileNavigation />
    </div>
  );
}
