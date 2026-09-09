import * as React from 'react';
import { Link } from 'react-router-dom';
import { BookOpen } from 'lucide-react';
import { api } from '../lib/api';
import { Card } from '../components/ui/Card';
import { Button } from '../components/ui/Button';
import { WelcomeSection } from '../components/dashboard/WelcomeSection';
import { ContinueLearning } from '../components/dashboard/ContinueLearning';
import { CourseCard } from '../components/dashboard/CourseCard';
import { RecentActivity } from '../components/dashboard/RecentActivity';
import { QuickActions } from '../components/dashboard/QuickActions';
import { StudentInfoCard } from '../components/dashboard/StudentInfoCard';
import { DashboardSkeleton } from '../components/dashboard/DashboardSkeleton';

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

function chapterLabel(item: DashboardData['recentActivity'][number] | undefined): string | null {
  if (!item) return null;
  const parts: string[] = [];
  if (item.unitId) parts.push(`Unit ${item.unitId}`);
  if (item.chapterId) parts.push(`Chapter ${item.chapterId}`);
  return parts.length ? parts.join(' · ') : null;
}

export default function StudentDashboard() {
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

  // ── Loading (§23) ───────────────────────────────────────────────────────
  if (state === 'loading') return <DashboardSkeleton />;

  // ── Error (§24) ─────────────────────────────────────────────────────────
  if (state === 'error') {
    return (
      <div className="mx-auto w-full max-w-6xl px-4 py-16 sm:px-6 lg:px-8">
        <Card variant="solid" padding="lg" className="mx-auto max-w-md text-center">
          <p className="text-lg font-semibold text-ink-900">Unable to load your dashboard.</p>
          <p className="mt-2 text-sm text-ink-500">
            Something went wrong. Your session is safe — please try again.
          </p>
          <Button className="mt-6" onClick={load}>
            Try Again
          </Button>
        </Card>
      </div>
    );
  }

  if (!data) return null;

  const { student, currentCourse, courses, progress, recentActivity } = data;

  // ── New-student empty state (§25): no enrollments at all ────────────────
  if (courses.length === 0) {
    return (
      <div className="mx-auto w-full max-w-6xl px-4 py-10 sm:px-6 lg:px-8">
        <WelcomeSection
          fullName={student.fullName}
          program={student.program?.name}
          semester={student.semester?.name}
          rollNumber={student.rollNumber}
        />
        <Card variant="solid" padding="lg" className="mt-8 text-center">
          <span className="mx-auto flex h-12 w-12 items-center justify-center rounded-xl bg-dg-50" aria-hidden>
            <BookOpen size={22} className="text-dg-700" />
          </span>
          <h2 className="mt-4 text-lg font-semibold text-ink-900">Welcome to Digi Guru</h2>
          <p className="mx-auto mt-2 max-w-md text-sm text-ink-500">
            You have no courses assigned yet. Once your study centre assigns your semester courses, they
            will appear here and you can start learning right away.
          </p>
          <Button className="mt-6" onClick={load} variant="secondary">
            Refresh
          </Button>
        </Card>
      </div>
    );
  }

  const currentProgress = currentCourse
    ? (progress.find((p) => p.courseId === currentCourse.id)?.percent ?? 0)
    : 0;
  const lastActivity = recentActivity[0];

  return (
    <div className="mx-auto w-full max-w-6xl px-4 py-8 sm:px-6 lg:px-8">
      {/* ── Greeting (§7) ─────────────────────────────────────────────────── */}
      <WelcomeSection
        fullName={student.fullName}
        program={student.program?.name}
        semester={student.semester?.name}
        rollNumber={student.rollNumber}
      />

      <div className="mt-8 grid gap-6 lg:grid-cols-3">
        {/* ── Main column ─────────────────────────────────────────────────── */}
        <div className="space-y-6 lg:col-span-2">
          {/* Continue Learning (§8) — primary card */}
          {currentCourse ? (
            <ContinueLearning
              courseName={currentCourse.name}
              chapterLabel={chapterLabel(lastActivity)}
              progressPercent={currentProgress}
              lastStudiedIso={lastActivity?.startedAt ?? null}
            />
          ) : (
            <Card variant="solid" padding="md">
              <p className="text-xs font-semibold uppercase tracking-wider text-dg-700">Continue Learning</p>
              <p className="mt-3 text-sm text-ink-500">
                No learning sessions yet — open a course below to begin.
              </p>
            </Card>
          )}

          {/* My Courses (§10) */}
          <section>
            <div className="flex items-center justify-between">
              <h2 className="text-base font-semibold text-ink-900">My Courses</h2>
              <Link to="/student/courses" className="text-sm font-semibold text-dg-700 hover:text-dg-900">
                View all
              </Link>
            </div>
            <div className="mt-4 grid gap-4 sm:grid-cols-2">
              {courses.map((course) => (
                <CourseCard
                  key={course.id}
                  course={course}
                  progressPercent={progress.find((p) => p.courseId === course.id)?.percent ?? 0}
                />
              ))}
            </div>
          </section>

          {/* Recent Activity (§13) */}
          <RecentActivity items={recentActivity} />
        </div>

        {/* ── Side column ─────────────────────────────────────────────────── */}
        <div className="space-y-6">
          <StudentInfoCard
            fullName={student.fullName}
            rollNumber={student.rollNumber}
            program={student.program?.name}
            semester={student.semester?.name}
          />
          <QuickActions />
        </div>
      </div>
    </div>
  );
}
