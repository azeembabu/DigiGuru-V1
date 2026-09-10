import { Link } from 'react-router-dom';
import { ArrowLeft } from 'lucide-react';

/**
 * Page header: back navigation, the page title and a one-line description.
 * Kept deliberately plain — no breadcrumbs or decorative chrome.
 */
export function ProfileHeader() {
  return (
    <header className="mb-6">
      <Link
        to="/student/dashboard"
        className="inline-flex items-center gap-1.5 rounded-lg text-sm font-medium text-ink-500 transition-colors hover:text-ink-900 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-dg-600 focus-visible:ring-offset-2"
      >
        <ArrowLeft size={16} aria-hidden />
        Back to dashboard
      </Link>

      <h1 className="mt-3 font-display text-2xl font-semibold tracking-tight text-ink-900 sm:text-[28px]">
        My Profile
      </h1>
      <p className="mt-1 text-sm text-ink-500">Manage your personal and academic information.</p>
    </header>
  );
}
