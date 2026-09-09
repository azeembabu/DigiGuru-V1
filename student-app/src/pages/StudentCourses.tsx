import * as React from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { api } from '../lib/api';
import { Card } from '../components/ui/Card';
import { Button } from '../components/ui/Button';

interface CourseDto {
  id: string;
  name: string;
  code: string;
  description: string | null;
  enrollment: { status: string; assignedAt: string } | null;
}

type LoadState = 'loading' | 'ready' | 'error';

/** Button-based course selection (§11–§12). Voice selection comes later. */
export default function StudentCourses() {
  const navigate = useNavigate();
  const [courses, setCourses] = React.useState<CourseDto[]>([]);
  const [state, setState] = React.useState<LoadState>('loading');
  const [selectedId, setSelectedId] = React.useState<string | null>(null);
  const [actionError, setActionError] = React.useState<string | null>(null);
  const [isSelecting, setIsSelecting] = React.useState(false);

  const load = React.useCallback(async () => {
    setState('loading');
    try {
      const { data: res } = await api.get('/student/courses');
      setCourses(res.data);
      // Preselect the currently ACTIVE enrollment if any
      const active = (res.data as CourseDto[]).find((c) => c.enrollment?.status === 'ACTIVE');
      setSelectedId(active?.id ?? null);
      setState('ready');
    } catch {
      setState('error');
    }
  }, []);

  React.useEffect(() => {
    load();
  }, [load]);

  async function handleSelect(course: CourseDto) {
    setActionError(null);
    setIsSelecting(true);
    try {
      await api.post('/student/courses/select', { courseId: course.id });
      navigate('/student/dashboard');
    } catch (err: unknown) {
      const msg =
        (err as { response?: { data?: { message?: string } } })?.response?.data?.message ??
        'Could not select this course. Please try again.';
      setActionError(msg);
      setIsSelecting(false);
    }
  }

  if (state === 'loading') {
    return (
      <div className="mx-auto max-w-4xl px-4 py-10 sm:px-6 lg:px-8">
        <div className="animate-pulse space-y-4">
          <div className="h-10 w-56 rounded-xl bg-slate-200/70" />
          {[0, 1, 2].map((i) => (
            <div key={i} className="h-24 rounded-2xl bg-slate-200/60" />
          ))}
        </div>
        <p className="mt-6 text-center text-sm text-ink-500">Loading your courses…</p>
      </div>
    );
  }

  if (state === 'error') {
    return (
      <div className="mx-auto max-w-4xl px-4 py-16 sm:px-6 lg:px-8">
        <Card variant="solid" padding="lg" className="mx-auto max-w-md text-center">
          <p className="text-lg font-semibold text-ink-900">Unable to load your courses.</p>
          <Button className="mt-6" onClick={load}>
            Try Again
          </Button>
        </Card>
      </div>
    );
  }

  return (
    <div className="mx-auto max-w-4xl px-4 py-8 sm:px-6 lg:px-8">
      <div className="flex items-center justify-between gap-4">
        <div>
          <p className="text-xs font-medium uppercase tracking-[0.14em] text-dg-700/80">My Courses</p>
          <h1 className="mt-1 font-display text-2xl font-medium text-ink-900">Choose your course</h1>
          <p className="mt-1 text-sm text-ink-500">
            Courses are matched to your program and semester. Your selection becomes your active learning context.
          </p>
        </div>
        <Link
          to="/student/dashboard"
          className="hidden shrink-0 rounded-full border border-slate-200 bg-white px-5 py-2.5 text-sm font-medium text-ink-700 transition hover:bg-slate-50 focus-ring sm:inline-flex"
        >
          ← Dashboard
        </Link>
      </div>

      {actionError && (
        <div className="mt-6 rounded-xl border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-800" role="alert">
          {actionError}
        </div>
      )}

      {courses.length === 0 ? (
        <Card variant="subtle" padding="lg" className="mt-8 border-dashed text-center">
          <p className="text-sm font-medium text-ink-900">No courses available yet.</p>
          <p className="mt-1 text-sm text-ink-500">
            Courses for your program and semester will appear here once your LSC/admin adds them.
          </p>
        </Card>
      ) : (
        <ul className="mt-8 space-y-4">
          {courses.map((course) => {
            const isActive = course.enrollment?.status === 'ACTIVE';
            const isSelected = selectedId === course.id;
            return (
              <li key={course.id}>
                <button
                  type="button"
                  onClick={() => handleSelect(course)}
                  disabled={isSelecting}
                  className={[
                    'w-full rounded-2xl border p-5 text-left transition focus-ring',
                    isSelected
                      ? 'border-dg-600 bg-emerald-50/70 ring-2 ring-dg-600/20'
                      : 'border-slate-200 bg-white hover:border-emerald-300 hover:bg-emerald-50/40',
                  ].join(' ')}
                >
                  <div className="flex items-start justify-between gap-4">
                    <div>
                      <div className="flex flex-wrap items-center gap-2">
                        <p className="text-base font-semibold text-ink-900">{course.name}</p>
                        <span className="rounded-full bg-slate-100 px-2 py-0.5 text-[11px] font-medium text-ink-500">
                          {course.code}
                        </span>
                        {isActive && (
                          <span className="rounded-full bg-emerald-100 px-2 py-0.5 text-[11px] font-medium text-emerald-800">
                            Active
                          </span>
                        )}
                      </div>
                      {course.description && (
                        <p className="mt-1.5 text-sm leading-relaxed text-ink-500">{course.description}</p>
                      )}
                    </div>
                    <span
                      className={[
                        'mt-1 inline-flex h-5 w-5 shrink-0 items-center justify-center rounded-full border',
                        isSelected ? 'border-dg-600 bg-dg-600' : 'border-slate-300 bg-white',
                      ].join(' ')}
                      aria-hidden
                    >
                      {isSelected && (
                        <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden>
                          <path d="M2.5 6.5L5 9l4.5-5" stroke="white" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
                        </svg>
                      )}
                    </span>
                  </div>
                </button>
              </li>
            );
          })}
        </ul>
      )}

      {isSelecting && <p className="mt-4 text-center text-sm text-ink-500">Selecting your course…</p>}
    </div>
  );
}
