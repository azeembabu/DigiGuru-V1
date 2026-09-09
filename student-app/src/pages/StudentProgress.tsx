import * as React from 'react';
import { api } from '../lib/api';
import { Card } from '../components/ui/Card';
import { Button } from '../components/ui/Button';

/**
 * Progress page (spec §12): overall % + per-unit breakdown.
 * Real, derived data only — units with no sessions show "Not Started".
 */

interface ProgressRow {
  courseId: string;
  courseName: string;
  percent: number;
  units: Array<{ unitId: string; status: string; percent: number }>;
}

type LoadState = 'loading' | 'ready' | 'error';

const UNIT_STATUS_STYLES: Record<string, string> = {
  Completed: 'bg-dg-50 text-dg-700',
  'In Progress': 'bg-amber-50 text-amber-700',
  'Not Started': 'bg-slate-100 text-ink-500',
};

export default function StudentProgress() {
  const [rows, setRows] = React.useState<ProgressRow[] | null>(null);
  const [state, setState] = React.useState<LoadState>('loading');

  const load = React.useCallback(async () => {
    setState('loading');
    try {
      const { data: res } = await api.get('/student/progress');
      setRows(res.data);
      setState('ready');
    } catch {
      setState('error');
    }
  }, []);

  React.useEffect(() => {
    load();
  }, [load]);

  if (state === 'loading') {
    return (
      <div className="mx-auto w-full max-w-6xl px-4 py-8 sm:px-6 lg:px-8" aria-busy="true">
        <span className="sr-only">Loading your progress…</span>
        <div className="animate-pulse space-y-6">
          <div className="h-8 w-48 rounded-lg bg-slate-200" />
          <div className="h-28 rounded-xl bg-slate-100" />
          <div className="h-64 rounded-xl bg-slate-100" />
        </div>
      </div>
    );
  }

  if (state === 'error') {
    return (
      <div className="mx-auto w-full max-w-6xl px-4 py-16 sm:px-6 lg:px-8">
        <Card variant="solid" padding="lg" className="mx-auto max-w-md text-center">
          <p className="text-lg font-semibold text-ink-900">Unable to load your progress.</p>
          <p className="mt-2 text-sm text-ink-500">Please try again.</p>
          <Button className="mt-6" onClick={load}>
            Try Again
          </Button>
        </Card>
      </div>
    );
  }

  if (!rows) return null;

  // Overall percent: average across enrolled courses with any real data.
  const overall =
    rows.length > 0 ? Math.round(rows.reduce((acc, r) => acc + r.percent, 0) / rows.length) : 0;

  return (
    <div className="mx-auto w-full max-w-6xl px-4 py-8 sm:px-6 lg:px-8">
      <h1 className="text-[26px] font-semibold leading-tight tracking-tight text-ink-900 sm:text-3xl">
        My Progress
      </h1>

      {rows.length === 0 ? (
        <Card variant="solid" padding="lg" className="mt-8 text-center">
          <p className="text-sm font-medium text-ink-900">No courses assigned yet.</p>
          <p className="mt-1 text-sm text-ink-500">
            Your progress will appear here once you start learning.
          </p>
        </Card>
      ) : (
        <>
          {/* Overall summary */}
          <Card variant="solid" padding="md" className="mt-6">
            <div className="flex items-center justify-between text-sm font-medium text-ink-700">
              <span>Overall completion</span>
              <span className="text-lg font-semibold text-ink-900">{overall}%</span>
            </div>
            <div
              className="mt-2 h-2 overflow-hidden rounded-full bg-slate-100"
              role="progressbar"
              aria-valuenow={overall}
              aria-valuemin={0}
              aria-valuemax={100}
            >
              <div className="h-full rounded-full bg-dg-600" style={{ width: `${Math.min(overall, 100)}%` }} />
            </div>
          </Card>

          {/* Per-course breakdown */}
          <div className="mt-6 space-y-6">
            {rows.map((row) => (
              <Card key={row.courseId} variant="solid" padding="md">
                <div className="flex items-center justify-between">
                  <h2 className="text-base font-semibold text-ink-900">{row.courseName}</h2>
                  <span className="text-sm font-semibold text-ink-700">{row.percent}%</span>
                </div>

                {row.units.length === 0 ? (
                  <p className="mt-4 text-sm text-ink-500">
                    No units started yet — your progress will appear here as you study.
                  </p>
                ) : (
                  <ul className="mt-4 space-y-3">
                    {row.units.map((unit) => (
                      <li key={unit.unitId} className="flex items-center gap-4">
                        <span
                          className={`w-24 shrink-0 rounded-md px-2 py-1 text-center text-[11px] font-semibold ${UNIT_STATUS_STYLES[unit.status] ?? UNIT_STATUS_STYLES['Not Started']}`}
                        >
                          {unit.status}
                        </span>
                        <div className="min-w-0 flex-1">
                          <div className="flex items-center justify-between text-xs font-medium text-ink-700">
                            <span className="truncate">Unit {unit.unitId}</span>
                            <span>{unit.percent}%</span>
                          </div>
                          <div className="mt-1 h-1.5 overflow-hidden rounded-full bg-slate-100">
                            <div
                              className="h-full rounded-full bg-dg-600"
                              style={{ width: `${Math.min(unit.percent, 100)}%` }}
                            />
                          </div>
                        </div>
                      </li>
                    ))}
                  </ul>
                )}
              </Card>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
