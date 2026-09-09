import * as React from 'react';
import { Link } from 'react-router-dom';
import { api } from '../lib/api';
import { useAuthStore } from '../store/authStore';
import { Card } from '../components/ui/Card';
import { Button } from '../components/ui/Button';

interface RefDto {
  id: string;
  name: string;
  code: string;
}

interface DashboardData {
  student: {
    fullName: string;
    rollNumber: string;
    email: string;
    program: RefDto | null;
    semester: { id: string; name: string; semester_number: number } | null;
    lsc: RefDto | null;
    lastLoginAt: string | null;
  };
  currentCourse: { id: string; name: string; code: string; description?: string | null } | null;
  courses: Array<{ id: string; name: string; code: string; description?: string | null }>;
  progress: Array<{
    courseId: string;
    courseName: string;
    percent: number;
    units: Array<{ unitId: string; status: string; percent: number }>;
  }>;
  recentActivity: Array<{
    id: string;
    courseName: string;
    unitId: string | null;
    chapterId: string | null;
    startedAt: string;
    durationSeconds: number;
    completionStatus: string;
  }>;
}

type LoadState = 'loading' | 'ready' | 'error';

function formatDuration(seconds: number): string {
  if (seconds < 60) return `${seconds}s`;
  const m = Math.floor(seconds / 60);
  return `${m}m`;
}

function formatDay(iso: string): string {
  const d = new Date(iso);
  const today = new Date();
  const yesterday = new Date(today);
  yesterday.setDate(today.getDate() - 1);
  const sameDay = (a: Date, b: Date) =>
    a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();
  if (sameDay(d, today)) return 'Today';
  if (sameDay(d, yesterday)) return 'Yesterday';
  return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
}

export default function StudentDashboard() {
  const storeUser = useAuthStore((s) => s.user);
  const [data, setData] = React.useState<DashboardData | null>(null);
  const [state, setState] = React.useState<LoadState>('loading');

  const load = React.useCallback(async () => {
    setState('loading');
    try {
      const { data: res } = await api.get('/student/dashboard');
      setData(res.data);
      setState('ready');
    } catch {
      setState('error');
    }
  }, []);

  React.useEffect(() => {
    load();
  }, [load]);

  // ── Loading state (§31) ────────────────────────────────────────────────
  if (state === 'loading') {
    return (
      <div className="mx-auto max-w-6xl px-4 py-10 sm:px-6 lg:px-8">
        <div className="animate-pulse space-y-6">
          <div className="h-36 rounded-[28px] bg-slate-200/70" />
          <div className="grid gap-6 lg:grid-cols-3">
            <div className="h-64 rounded-2xl bg-slate-200/60" />
            <div className="h-64 rounded-2xl bg-slate-200/60 lg:col-span-2" />
          </div>
        </div>
        <p className="mt-6 text-center text-sm text-ink-500">Loading your dashboard…</p>
      </div>
    );
  }

  // ── Error state (§32) ──────────────────────────────────────────────────
  if (state === 'error') {
    return (
      <div className="mx-auto max-w-6xl px-4 py-16 sm:px-6 lg:px-8">
        <Card variant="solid" padding="lg" className="mx-auto max-w-md text-center">
          <p className="text-lg font-semibold text-ink-900">Unable to load your dashboard.</p>
          <p className="mt-2 text-sm text-ink-500">Something went wrong. Your session is safe — please try again.</p>
          <Button className="mt-6" onClick={load}>
            Try Again
          </Button>
        </Card>
      </div>
    );
  }

  if (!data) return null;

  const { student, currentCourse, courses, progress, recentActivity } = data;
  const firstName = student.fullName.split(' ')[0] ?? student.fullName;

  return (
    <div className="mx-auto max-w-6xl px-4 py-6 sm:px-6 lg:px-8">
      {/* ── Identity header (§5–§8) ───────────────────────────────────────── */}
      <div className="overflow-hidden rounded-[28px] border border-emerald-100 bg-gradient-to-br from-emerald-50 via-white to-teal-50/60 p-6 shadow-soft sm:p-8">
        <div className="flex flex-col gap-6 sm:flex-row sm:items-start sm:justify-between">
          <div>
            <p className="text-xs font-medium uppercase tracking-[0.14em] text-dg-700/80">Dashboard</p>
            <h1 className="mt-2 font-display text-[26px] font-medium leading-tight text-ink-900 sm:text-[30px]">
              Welcome back, <span className="text-dg-900">{firstName}</span>
            </h1>
            <div className="mt-4 flex flex-wrap gap-2">
              <span className="inline-flex items-center gap-1.5 rounded-full bg-white px-3 py-1.5 text-xs font-medium text-ink-700 shadow-sm ring-1 ring-slate-200">
                <span className="h-2 w-2 rounded-full bg-emerald-500" aria-hidden />
                {student.program?.name ?? '—'}
              </span>
              <span className="inline-flex items-center rounded-full bg-white px-3 py-1.5 text-xs font-medium text-ink-700 shadow-sm ring-1 ring-slate-200">
                {student.semester?.name ?? '—'}
              </span>
              <span className="inline-flex items-center rounded-full bg-white px-3 py-1.5 text-xs font-medium text-ink-700 shadow-sm ring-1 ring-slate-200">
                LSC: {student.lsc?.name ?? '—'}
              </span>
              <span className="inline-flex items-center rounded-full bg-dg-900 px-3 py-1.5 text-xs font-medium text-white shadow-sm">
                Roll No: {student.rollNumber}
              </span>
            </div>
          </div>
          <div className="flex shrink-0 flex-col gap-3 sm:items-end">
            <Link
              to="/student/profile"
              className="inline-flex items-center justify-center rounded-full border border-slate-200 bg-white px-5 py-2.5 text-sm font-medium text-ink-700 shadow-sm transition hover:bg-slate-50 focus-ring"
            >
              Profile
            </Link>
            <p className="text-xs text-ink-400">Your data is visible only to you.</p>
          </div>
        </div>
      </div>

      <div className="mt-6 grid gap-6 lg:grid-cols-3">
        {/* ── Continue Learning (§9) ──────────────────────────────────────── */}
        <Card variant="solid" padding="md" className="lg:col-span-2">
          <h2 className="text-base font-semibold text-ink-900">Continue Learning</h2>
          {currentCourse ? (
            <div className="mt-4 rounded-2xl border border-emerald-100 bg-gradient-to-br from-emerald-50/80 to-white p-5">
              <p className="text-xs font-medium uppercase tracking-wide text-dg-700/80">
                {student.program?.name ?? 'Course'}
              </p>
              <p className="mt-1.5 text-lg font-semibold text-ink-900">{currentCourse.name}</p>
              {currentCourse.description && (
                <p className="mt-1 line-clamp-2 text-sm text-ink-500">{currentCourse.description}</p>
              )}
              {/* Progress for the current course, only if real data exists */}
              {(() => {
                const p = progress.find((row) => row.courseId === currentCourse.id);
                return p && p.percent > 0 ? (
                  <div className="mt-4">
                    <div className="flex items-center justify-between text-xs font-medium text-ink-600">
                      <span>Progress</span>
                      <span>{p.percent}%</span>
                    </div>
                    <div className="mt-1.5 h-2 overflow-hidden rounded-full bg-slate-200">
                      <div className="h-full rounded-full bg-dg-600 transition-all" style={{ width: `${p.percent}%` }} />
                    </div>
                  </div>
                ) : null;
              })()}
              <div className="mt-5 flex flex-wrap gap-3">
                <Link
                  to="/student/courses"
                  className="inline-flex items-center justify-center rounded-full bg-dg-900 px-6 py-2.5 text-sm font-medium text-white shadow-[0_4px_16px_rgba(14,75,58,0.22)] transition hover:bg-dg-950 focus-ring"
                >
                  Continue Class →
                </Link>
                {recentActivity[0] && (
                  <span className="inline-flex items-center text-xs text-ink-400">
                    Last studied: {formatDay(recentActivity[0].startedAt)}
                  </span>
                )}
              </div>
            </div>
          ) : (
            /* Empty state (§30) — no fake progress */
            <div className="mt-4 rounded-2xl border border-dashed border-slate-300 bg-slate-50/60 p-6 text-center">
              <p className="text-sm font-medium text-ink-900">No learning sessions yet.</p>
              <p className="mt-1 text-sm text-ink-500">Start your first class to begin your learning journey.</p>
              <Link
                to="/student/courses"
                className="mt-4 inline-flex items-center justify-center rounded-full bg-dg-900 px-6 py-2.5 text-sm font-medium text-white shadow-sm transition hover:bg-dg-950 focus-ring"
              >
                View Courses
              </Link>
            </div>
          )}
        </Card>

        {/* ── Identity summary (§5) ───────────────────────────────────────── */}
        <Card variant="solid" padding="md">
          <h2 className="text-sm font-semibold uppercase tracking-wide text-ink-700">Your academic identity</h2>
          <dl className="mt-4 space-y-3 text-sm">
            <div className="flex justify-between gap-4 border-b border-slate-100 py-2.5">
              <dt className="text-ink-500">Full name</dt>
              <dd className="font-medium text-ink-900">{student.fullName}</dd>
            </div>
            <div className="flex justify-between gap-4 border-b border-slate-100 py-2.5">
              <dt className="text-ink-500">Roll Number</dt>
              <dd className="font-mono text-xs font-medium text-ink-900">{student.rollNumber}</dd>
            </div>
            <div className="flex justify-between gap-4 border-b border-slate-100 py-2.5">
              <dt className="text-ink-500">Program</dt>
              <dd className="font-medium text-ink-900">{student.program?.name ?? '—'}</dd>
            </div>
            <div className="flex justify-between gap-4 border-b border-slate-100 py-2.5">
              <dt className="text-ink-500">Semester</dt>
              <dd className="font-medium text-ink-900">{student.semester?.name ?? '—'}</dd>
            </div>
            <div className="flex justify-between gap-4 py-2.5">
              <dt className="text-ink-500">LSC</dt>
              <dd className="font-medium text-ink-900">{student.lsc?.name ?? '—'}</dd>
            </div>
          </dl>
        </Card>
      </div>

      <div className="mt-6 grid gap-6 lg:grid-cols-3">
        {/* ── Recent Activity (§14) ───────────────────────────────────────── */}
        <Card variant="solid" padding="md" className="lg:col-span-2">
          <h2 className="text-base font-semibold text-ink-900">Recent Activity</h2>
          {recentActivity.length === 0 ? (
            <p className="mt-4 text-sm text-ink-500">No activity yet — your learning history will appear here.</p>
          ) : (
            <ul className="mt-4 divide-y divide-slate-100">
              {recentActivity.map((a) => (
                <li key={a.id} className="flex items-center justify-between gap-4 py-3">
                  <div>
                    <p className="text-sm font-medium text-ink-900">{a.courseName}</p>
                    <p className="text-xs text-ink-500">
                      {a.unitId ? `Unit ${a.unitId}` : 'Course session'}
                      {a.chapterId ? ` · Chapter ${a.chapterId}` : ''}
                    </p>
                  </div>
                  <div className="text-right">
                    <p className="text-xs font-medium text-ink-700">{formatDay(a.startedAt)}</p>
                    {a.durationSeconds > 0 && (
                      <p className="text-xs text-ink-400">{formatDuration(a.durationSeconds)}</p>
                    )}
                  </div>
                </li>
              ))}
            </ul>
          )}
        </Card>

        {/* ── Quick Access (§18) ──────────────────────────────────────────── */}
        <Card variant="solid" padding="md">
          <h2 className="text-base font-semibold text-ink-900">Quick Access</h2>
          <div className="mt-4 grid grid-cols-2 gap-3">
            {[
              { label: 'Courses', to: '/student/courses', ready: true },
              { label: 'Profile', to: '/student/profile', ready: true },
              { label: 'Notes', to: '/student/settings', ready: false },
              { label: 'Tests', to: '/student/settings', ready: false },
            ].map((item) =>
              item.ready ? (
                <Link
                  key={item.label}
                  to={item.to}
                  className="rounded-2xl border border-slate-100 bg-slate-50/60 p-4 text-sm font-semibold text-ink-900 transition hover:border-emerald-200 hover:bg-emerald-50/50 focus-ring"
                >
                  {item.label} →
                </Link>
              ) : (
                <div
                  key={item.label}
                  className="rounded-2xl border border-slate-100 bg-slate-50/60 p-4 text-sm font-semibold text-ink-400"
                  aria-disabled
                >
                  {item.label}
                  <span className="mt-1 block text-[11px] font-normal text-ink-400">Coming soon</span>
                </div>
              ),
            )}
          </div>
          <Link
            to="/student/settings"
            className="mt-4 inline-flex text-sm font-medium text-dg-700 hover:text-dg-900 hover:underline focus-ring rounded"
          >
            Settings →
          </Link>
        </Card>
      </div>

      {/* Greeting fallback for users arriving before profile loads */}
      {!storeUser && <p className="mt-6 text-center text-xs text-ink-400">Syncing your profile…</p>}
    </div>
  );
}
