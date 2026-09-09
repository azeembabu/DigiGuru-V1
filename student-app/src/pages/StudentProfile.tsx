import * as React from 'react';
import { Link } from 'react-router-dom';
import { useAuthStore } from '../store/authStore';
import { Card } from '../components/ui/Card';

function Field({ label, value, readOnly }: { label: string; value: string; readOnly?: boolean }) {
  return (
    <div className="space-y-1.5">
      <p className="text-xs font-medium uppercase tracking-wide text-ink-500">
        {label} {readOnly && <span className="ml-1 rounded bg-slate-100 px-1.5 py-0.5 text-[10px] font-medium text-ink-500">read-only</span>}
      </p>
      <div
        className={[
          'rounded-xl border px-3.5 py-3 text-sm',
          readOnly ? 'border-slate-200 bg-slate-50 text-ink-700' : 'border-slate-200 bg-white text-ink-900',
        ].join(' ')}
      >
        {value || '—'}
      </div>
    </div>
  );
}

export default function StudentProfile() {
  const user = useAuthStore((s) => s.user);

  return (
    <div className="mx-auto max-w-6xl px-4 py-6 sm:px-6 lg:px-8">
      <div className="mb-6 flex flex-wrap items-center justify-between gap-4">
        <div>
          <h1 className="font-display text-2xl font-semibold tracking-tight text-ink-900 sm:text-[28px]">Profile</h1>
          <p className="mt-1 text-sm text-ink-500">Your personal and academic information.</p>
        </div>
        <Link to="/student/dashboard" className="inline-flex rounded-full border border-slate-200 bg-white px-4 py-2 text-sm font-medium text-ink-700 hover:bg-slate-50 focus-ring">
          ← Back to dashboard
        </Link>
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        <Card variant="solid" padding="md">
          <h2 className="flex items-center gap-2 text-sm font-semibold uppercase tracking-wide text-ink-700">
            <span className="h-2 w-2 rounded-full bg-dg-500" aria-hidden /> Personal information
          </h2>
          <div className="mt-5 grid gap-4">
            <Field label="Full name" value={user?.fullName ?? ''} />
            <Field label="Roll Number" value={user?.rollNumber ?? ''} readOnly />
            <div className="grid gap-4 sm:grid-cols-2">
              <Field label="Email" value={user?.email ?? ''} />
              <Field label="Phone number" value={user?.phoneNumber ?? ''} />
            </div>
            <p className="text-xs leading-relaxed text-ink-400">
              Roll Number is your permanent identity and cannot be changed. Contact your LSC for corrections.
            </p>
          </div>
        </Card>

        <Card variant="solid" padding="md">
          <h2 className="flex items-center gap-2 text-sm font-semibold uppercase tracking-wide text-ink-700">
            <span className="h-2 w-2 rounded-full bg-teal-500" aria-hidden /> Academic information
          </h2>
          <div className="mt-5 grid gap-4">
            <Field label="Program" value={user?.program ?? ''} readOnly />
            <div className="grid gap-4 sm:grid-cols-2">
              <Field label="Semester" value={user?.semester ?? ''} readOnly />
              <Field label="LSC" value={user?.lsc ?? ''} readOnly />
            </div>
            <div className="rounded-xl border border-dashed border-slate-200 bg-slate-50/60 px-4 py-3">
              <p className="text-xs font-medium text-ink-700">Academic details are read-only</p>
              <p className="mt-1 text-xs leading-relaxed text-ink-500">
                Program, Semester, and LSC are set at admission. To update them, please contact your Learner Support Centre.
              </p>
            </div>
          </div>
        </Card>
      </div>

      <Card variant="subtle" padding="md" className="mt-6 border-dashed">
        <p className="text-sm font-medium text-ink-900">Privacy</p>
        <p className="mt-1 text-sm leading-relaxed text-ink-500">
          Your profile is visible only to you. No other student can see your data. Everything is separated by Roll Number.
        </p>
      </Card>
    </div>
  );
}
