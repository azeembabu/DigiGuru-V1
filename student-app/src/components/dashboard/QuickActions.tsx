import { Link } from 'react-router-dom';
import { BookOpen, FileText, ClipboardList, TrendingUp } from 'lucide-react';

/**
 * Quick Access (spec §15): limited to 4 items, icons + text.
 * Notes/Tests are Coming Soon — no fake navigation.
 */

const READY_ITEMS = [
  { label: 'Courses', to: '/student/courses', icon: BookOpen, ready: true },
  { label: 'Progress', to: '/student/progress', icon: TrendingUp, ready: true },
  { label: 'Notes', to: '', icon: FileText, ready: false },
  { label: 'Tests', to: '', icon: ClipboardList, ready: false },
] as const;

export function QuickActions() {
  return (
    <section className="rounded-xl border border-slate-200 bg-white p-6 shadow-sm">
      <h2 className="text-base font-semibold text-ink-900">Quick Access</h2>
      <div className="mt-4 grid grid-cols-2 gap-3">
        {READY_ITEMS.map((item) =>
          item.ready ? (
            <Link
              key={item.label}
              to={item.to}
              className="flex items-center gap-2.5 rounded-lg border border-slate-200 px-4 py-3 text-sm font-semibold text-ink-900 transition-colors hover:border-dg-300 hover:bg-dg-50"
            >
              <item.icon size={18} className="text-dg-700" aria-hidden />
              {item.label}
            </Link>
          ) : (
            <div
              key={item.label}
              aria-disabled
              className="flex items-center gap-2.5 rounded-lg border border-slate-100 bg-slate-50 px-4 py-3 text-sm font-semibold text-slate-400"
            >
              <item.icon size={18} aria-hidden />
              <span>
                {item.label}
                <span className="block text-[11px] font-normal">Coming soon</span>
              </span>
            </div>
          ),
        )}
      </div>
    </section>
  );
}
