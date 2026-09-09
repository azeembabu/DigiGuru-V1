/**
 * Recent Activity (spec §13): vertical timeline with Today/Yesterday/date groups.
 */

import { formatDayLabel, formatDurationMinutes } from '../../lib/format';

interface ActivityItem {
  id: string;
  courseName: string;
  unitId: string | null;
  chapterId: string | null;
  startedAt: string;
  durationSeconds: number;
}

export function RecentActivity({ items }: { items: ActivityItem[] }) {
  return (
    <section className="rounded-xl border border-slate-200 bg-white p-6 shadow-sm">
      <h2 className="text-base font-semibold text-ink-900">Recent Activity</h2>

      {items.length === 0 ? (
        <p className="mt-3 text-sm text-ink-500">No activity yet — your learning history will appear here.</p>
      ) : (
        <ol className="mt-4 space-y-0">
          {items.map((item, idx) => (
            <li key={item.id} className="relative flex gap-3 pb-5 last:pb-0">
              {/* Timeline dot + connector */}
              <span className="relative mt-1.5 flex h-2.5 w-2.5 shrink-0">
                <span className="absolute inline-flex h-full w-full rounded-full bg-dg-600" />
                {idx < items.length - 1 && (
                  <span className="absolute left-1/2 top-3 h-[calc(100%+8px)] w-px -translate-x-1/2 bg-slate-200" aria-hidden />
                )}
              </span>
              <div className="min-w-0">
                <p className="text-xs font-medium text-ink-500">{formatDayLabel(item.startedAt)}</p>
                <p className="mt-0.5 truncate text-sm font-medium text-ink-900">
                  {item.courseName}
                  {item.unitId ? ` — Unit ${item.unitId}` : ''}
                  {item.chapterId ? ` · Chapter ${item.chapterId}` : ''}
                </p>
                {(() => {
                  const duration = formatDurationMinutes(item.durationSeconds);
                  return duration ? <p className="text-xs text-ink-400">{duration}</p> : null;
                })()}
              </div>
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}
