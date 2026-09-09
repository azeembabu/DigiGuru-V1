import { Link } from 'react-router-dom';
import { Play } from 'lucide-react';
import { Button } from '../ui/Button';
import { formatDayLabel } from '../../lib/format';

/**
 * Continue Learning card (spec §8) — the primary component on the page.
 * Shows current course, last position, real progress bar, last-studied label.
 */

interface Props {
  courseName: string;
  chapterLabel: string | null;
  progressPercent: number;
  lastStudiedIso: string | null;
}

export function ContinueLearning({ courseName, chapterLabel, progressPercent, lastStudiedIso }: Props) {
  const lastStudied = lastStudiedIso ? formatDayLabel(lastStudiedIso) : null;
  return (
    <section className="rounded-xl border border-slate-200 bg-white p-6 shadow-sm">
      <p className="text-xs font-semibold uppercase tracking-wider text-dg-700">Continue Learning</p>
      <h2 className="mt-3 text-xl font-semibold text-ink-900">{courseName}</h2>
      {chapterLabel && <p className="mt-1 text-sm text-ink-500">{chapterLabel}</p>}

      <div className="mt-5">
        <div className="flex items-center justify-between text-xs font-medium text-ink-700">
          <span>Progress</span>
          <span>{progressPercent}%</span>
        </div>
        <div className="mt-1.5 h-2 overflow-hidden rounded-full bg-slate-100" role="progressbar" aria-valuenow={progressPercent} aria-valuemin={0} aria-valuemax={100}>
          <div className="h-full rounded-full bg-dg-600" style={{ width: `${Math.min(progressPercent, 100)}%` }} />
        </div>
      </div>

      <div className="mt-5 flex flex-wrap items-center justify-between gap-3">
        {lastStudied ? (
          <p className="text-xs text-ink-500">Last studied: {lastStudied}</p>
        ) : (
          <span />
        )}
        <Link to="/student/class">
          <Button size="lg">
            <Play size={16} aria-hidden /> Continue Class
          </Button>
        </Link>
      </div>
    </section>
  );
}
