import * as React from 'react';
import { useForm } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';
import { z } from 'zod';
import { Link, useNavigate } from 'react-router-dom';
import { api, clearTokens } from '../lib/api';
import { useAuthStore } from '../store/authStore';
import { Card } from '../components/ui/Card';
import { Button } from '../components/ui/Button';
import { Input } from '../components/ui/Input';

/* ── API shapes (GET /api/student/profile, GET /api/auth/sessions) ───────── */

interface ProfileData {
  id: string;
  full_name: string;
  roll_number: string;
  phone_number: string | null;
  created_at: string;
  updated_at: string;
  program: { id: string; name: string; code: string } | null;
  semester: { id: string; name: string; semester_number: number } | null;
  lsc: { id: string; name: string; code: string } | null;
  user: {
    id: string;
    email: string;
    role: string;
    status: string;
    created_at: string | null;
    last_login_at: string | null;
  };
}

interface SessionRow {
  id: string;
  deviceInfo: string | null;
  ipAddress: string | null;
  createdAt: string;
  expiresAt: string;
  revokedAt: string | null;
  isCurrent: boolean;
  isActive: boolean;
}

/* ── Validation (mirrors the backend's own rules) ──────────────────────────
   Passwords are never displayed or stored — inputs stay type="password" and
   values live only in form state for the lifetime of the request. */

const profileSchema = z.object({
  fullName: z.string().min(2, 'Name is too short').max(100, 'Name is too long'),
  phoneNumber: z
    .string()
    .min(7, 'Phone number is too short')
    .max(20, 'Phone number is too long')
    .regex(/^[0-9+\-\s]+$/, 'Phone number contains invalid characters'),
});
type ProfileFormValues = z.infer<typeof profileSchema>;

const passwordSchema = z
  .object({
    currentPassword: z.string().min(1, 'Current password is required'),
    newPassword: z
      .string()
      .min(8, 'At least 8 characters')
      .regex(/[A-Za-z]/, 'Include a letter')
      .regex(/[0-9]/, 'Include a number'),
    confirmPassword: z.string().min(1, 'Please confirm your new password'),
  })
  .refine((d) => d.newPassword === d.confirmPassword, {
    path: ['confirmPassword'],
    message: 'Passwords do not match',
  });
type PasswordFormValues = z.infer<typeof passwordSchema>;

/* ── Small helpers ───────────────────────────────────────────────────────── */

function getApiError(err: unknown, fallback: string): string {
  const e = err as {
    response?: { data?: { error?: string; details?: Array<{ message: string }> } };
    message?: string;
  };
  return e?.response?.data?.details?.[0]?.message ?? e?.response?.data?.error ?? e?.message ?? fallback;
}

function fmtDateTime(iso: string | null | undefined): string {
  if (!iso) return '—';
  return new Date(iso).toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' });
}

function fmtDate(iso: string | null | undefined): string {
  if (!iso) return '—';
  return new Date(iso).toLocaleDateString(undefined, { dateStyle: 'medium' });
}

function initials(name: string): string {
  const parts = name.split(/\s+/).filter(Boolean).slice(0, 2);
  return parts.map((p) => p[0]!.toUpperCase()).join('') || '?';
}

/** Human label from the stored user-agent string. Never shows raw tokens. */
function describeDevice(ua: string | null | undefined): string {
  if (!ua) return 'Unknown device';
  if (/curl/i.test(ua)) return 'API client';
  if (/Edg\//.test(ua)) return 'Edge browser';
  if (/OPR\//.test(ua)) return 'Opera browser';
  if (/Chrome\//.test(ua)) return 'Chrome browser';
  if (/Firefox\//.test(ua)) return 'Firefox browser';
  if (/Safari\//.test(ua)) return 'Safari browser';
  return ua.split(/\s+/)[0] ?? 'Unknown device';
}

function StatusBadge({ status }: { status: string }) {
  const tone =
    status === 'ACTIVE'
      ? 'bg-emerald-50 text-emerald-700 ring-emerald-200'
      : status === 'SUSPENDED'
        ? 'bg-amber-50 text-amber-700 ring-amber-200'
        : 'bg-slate-100 text-ink-500 ring-slate-200';
  const dot = status === 'ACTIVE' ? 'bg-emerald-500' : status === 'SUSPENDED' ? 'bg-amber-500' : 'bg-slate-400';
  return (
    <span className={`inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-xs font-medium ring-1 ${tone}`}>
      <span className={`h-1.5 w-1.5 rounded-full ${dot}`} aria-hidden />
      {status === 'ACTIVE' ? 'Active' : status === 'SUSPENDED' ? 'Suspended' : 'Inactive'}
    </span>
  );
}

function SectionTitle({ label, dot }: { label: string; dot: string }) {
  return (
    <h2 className="flex items-center gap-2 text-sm font-semibold uppercase tracking-wide text-ink-700">
      <span className={`h-2 w-2 rounded-full ${dot}`} aria-hidden />
      {label}
    </h2>
  );
}

function ReadOnlyField({ label, value, badge }: { label: string; value: string; badge?: string }) {
  return (
    <div className="space-y-1.5">
      <p className="text-xs font-medium uppercase tracking-wide text-ink-500">
        {label}
        {badge && (
          <span className="ml-2 rounded bg-slate-100 px-1.5 py-0.5 text-[10px] font-medium normal-case text-ink-500">
            {badge}
          </span>
        )}
      </p>
      <div className="rounded-xl border border-slate-200 bg-slate-50 px-3.5 py-3 text-sm text-ink-700">
        {value || '—'}
      </div>
    </div>
  );
}

function Skeleton({ className = '' }: { className?: string }) {
  return <div className={`animate-pulse rounded-xl bg-slate-100 ${className}`} aria-hidden />;
}

function AccountInfoRow({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-3 py-2.5">
      <span className="text-sm text-ink-500">{label}</span>
      <span className="text-sm font-medium text-ink-900 text-right">{children}</span>
    </div>
  );
}

/* ── Page ──────────────────────────────────────────────────────────────────
   Identity is always derived from the authenticated session (GET /student/
   profile on the backend reads req.user.id) — no student id is ever sent
   from here, so another student's data can never be requested. */

export default function StudentProfile() {
  const navigate = useNavigate();
  const setUser = useAuthStore((s) => s.setUser);
  const logoutStore = useAuthStore((s) => s.logout);

  const [profile, setProfile] = React.useState<ProfileData | null>(null);
  const [loading, setLoading] = React.useState(true);
  const [error, setError] = React.useState<string | null>(null);

  const [sessions, setSessions] = React.useState<SessionRow[] | null>(null);
  const [sessionsLoading, setSessionsLoading] = React.useState(true);
  const [sessionsError, setSessionsError] = React.useState<string | null>(null);
  const [revokingId, setRevokingId] = React.useState<string | null>(null);

  const [saveState, setSaveState] = React.useState<'idle' | 'saved' | 'error'>('idle');
  const [saveError, setSaveError] = React.useState<string | null>(null);
  const [pwState, setPwState] = React.useState<'idle' | 'saved'>('idle');
  const [pwError, setPwError] = React.useState<string | null>(null);
  const [isLoggingOut, setIsLoggingOut] = React.useState(false);

  const profileForm = useForm<ProfileFormValues>({
    resolver: zodResolver(profileSchema),
    defaultValues: { fullName: '', phoneNumber: '' },
  });
  const passwordForm = useForm<PasswordFormValues>({
    resolver: zodResolver(passwordSchema),
    defaultValues: { currentPassword: '', newPassword: '', confirmPassword: '' },
  });

  async function loadProfile() {
    setLoading(true);
    setError(null);
    try {
      const { data } = await api.get<{ success: boolean; data: ProfileData }>('/student/profile');
      setProfile(data.data);
      profileForm.reset({
        fullName: data.data.full_name ?? '',
        phoneNumber: data.data.phone_number ?? '',
      });
    } catch (err) {
      setError(getApiError(err, 'Could not load your profile. Please try again.'));
    } finally {
      setLoading(false);
    }
  }

  async function loadSessions() {
    setSessionsLoading(true);
    setSessionsError(null);
    try {
      const { data } = await api.get<{ success: boolean; data: SessionRow[] }>('/auth/sessions');
      setSessions(data.data);
    } catch (err) {
      setSessionsError(getApiError(err, 'Could not load your sessions. Please try again.'));
    } finally {
      setSessionsLoading(false);
    }
  }

  React.useEffect(() => {
    loadProfile();
    loadSessions();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function onSaveProfile(values: ProfileFormValues) {
    setSaveState('idle');
    setSaveError(null);
    try {
      // Only the approved editable fields are sent — roll number, program,
      // semester and LSC are never included, and the backend rejects any
      // other field with 400.
      const { data } = await api.patch<{ success: boolean; data: ProfileData }>('/student/profile', {
        full_name: values.fullName.trim(),
        phone_number: values.phoneNumber.trim(),
      });
      const updated = data.data;
      setProfile((prev) =>
        prev ? { ...prev, full_name: updated.full_name, phone_number: updated.phone_number, updated_at: updated.updated_at } : prev,
      );
      // Keep the header identity in sync.
      const prevUser = useAuthStore.getState().user;
      if (prevUser) {
        setUser({ ...prevUser, fullName: updated.full_name, phoneNumber: updated.phone_number ?? '' });
      }
      setSaveState('saved');
    } catch (err) {
      setSaveError(getApiError(err, 'Could not save your changes. Please try again.'));
      setSaveState('error');
    }
  }

  async function onChangePassword(values: PasswordFormValues) {
    setPwState('idle');
    setPwError(null);
    try {
      await api.post('/auth/change-password', {
        currentPassword: values.currentPassword,
        newPassword: values.newPassword,
      });
      passwordForm.reset();
      setPwState('saved');
      // Other devices were signed out server-side — refresh the list.
      loadSessions();
    } catch (err) {
      setPwError(getApiError(err, 'Could not change your password. Please try again.'));
    }
  }

  async function revokeSession(id: string) {
    setRevokingId(id);
    setSessionsError(null);
    try {
      await api.delete(`/auth/sessions/${id}`);
      setSessions((prev) => (prev ? prev.map((s) => (s.id === id ? { ...s, isActive: false, revokedAt: new Date().toISOString() } : s)) : prev));
    } catch (err) {
      setSessionsError(getApiError(err, 'Could not sign out that session. Please try again.'));
    } finally {
      setRevokingId(null);
    }
  }

  async function handleLogout() {
    setIsLoggingOut(true);
    try {
      await api.post('/auth/logout');
    } catch {
      // Session may already be expired — still clear local state
    } finally {
      clearTokens();
      logoutStore();
      navigate('/login', { replace: true });
    }
  }

  const currentSession = sessions?.find((s) => s.isCurrent) ?? null;
  const activeCount = sessions?.filter((s) => s.isActive).length ?? null;
  const status = profile?.user.status ?? '';

  /* ── Loading skeleton ──────────────────────────────────────────────────── */

  if (loading) {
    return (
      <div className="mx-auto max-w-6xl px-4 py-6 sm:px-6 lg:px-8">
        <Skeleton className="h-4 w-40" />
        <Skeleton className="mt-3 h-8 w-56" />
        <Skeleton className="mt-2 h-4 w-80" />
        <div className="mt-8 grid gap-6 lg:grid-cols-3">
          <div className="space-y-6 lg:col-span-2">
            <Card variant="solid" padding="md">
              <Skeleton className="h-4 w-48" />
              <div className="mt-6 grid gap-4 sm:grid-cols-2">
                <Skeleton className="h-16" />
                <Skeleton className="h-16" />
                <Skeleton className="h-16" />
                <Skeleton className="h-16" />
              </div>
            </Card>
            <Card variant="solid" padding="md">
              <Skeleton className="h-4 w-52" />
              <div className="mt-6 grid gap-4 sm:grid-cols-3">
                <Skeleton className="h-16" />
                <Skeleton className="h-16" />
                <Skeleton className="h-16" />
              </div>
            </Card>
            <Card variant="solid" padding="md">
              <Skeleton className="h-4 w-24" />
              <div className="mt-6 space-y-4">
                <Skeleton className="h-16" />
                <Skeleton className="h-16" />
                <Skeleton className="h-16" />
              </div>
            </Card>
          </div>
          <div className="space-y-6">
            <Card variant="solid" padding="md">
              <div className="flex items-center gap-4">
                <Skeleton className="h-16 w-16 rounded-full" />
                <div className="flex-1 space-y-2">
                  <Skeleton className="h-4 w-32" />
                  <Skeleton className="h-3 w-24" />
                </div>
              </div>
            </Card>
            <Card variant="solid" padding="md">
              <Skeleton className="h-4 w-40" />
              <div className="mt-5 space-y-3">
                <Skeleton className="h-8" />
                <Skeleton className="h-8" />
                <Skeleton className="h-8" />
              </div>
            </Card>
          </div>
        </div>
      </div>
    );
  }

  /* ── Error state ───────────────────────────────────────────────────────── */

  if (error || !profile) {
    return (
      <div className="mx-auto max-w-6xl px-4 py-6 sm:px-6 lg:px-8">
        <h1 className="font-display text-2xl font-semibold tracking-tight text-ink-900 sm:text-[28px]">My Profile</h1>
        <p className="mt-1 text-sm text-ink-500">Manage your personal and academic information</p>
        <Card variant="solid" padding="md" className="mt-8 text-center">
          <p className="text-base font-medium text-ink-900">We couldn&apos;t load your profile</p>
          <p className="mx-auto mt-1 max-w-md text-sm leading-relaxed text-ink-500">{error ?? 'Something went wrong.'}</p>
          <Button variant="secondary" className="mt-5" onClick={loadProfile}>
            Retry
          </Button>
        </Card>
      </div>
    );
  }

  /* ── Loaded state ──────────────────────────────────────────────────────── */

  const profileErrors = profileForm.formState.errors;
  const passwordErrors = passwordForm.formState.errors;

  return (
    <div className="mx-auto max-w-6xl px-4 py-6 sm:px-6 lg:px-8">
      {/* Header */}
      <div className="mb-8">
        <Link
          to="/student/dashboard"
          className="inline-flex items-center gap-1.5 text-sm font-medium text-ink-500 hover:text-ink-900 focus-ring rounded"
        >
          <span aria-hidden>←</span> Back to dashboard
        </Link>
        <h1 className="mt-3 font-display text-2xl font-semibold tracking-tight text-ink-900 sm:text-[28px]">My Profile</h1>
        <p className="mt-1 text-sm text-ink-500">Manage your personal and academic information</p>
      </div>

      <div className="grid items-start gap-6 lg:grid-cols-3">
        {/* ── Main column ─────────────────────────────────────────────────── */}
        <div className="space-y-6 lg:col-span-2">
          {/* Personal information */}
          <Card variant="solid" padding="md">
            <SectionTitle label="Personal information" dot="bg-dg-500" />
            <form onSubmit={profileForm.handleSubmit(onSaveProfile)} noValidate className="mt-6 space-y-5">
              <div className="grid gap-5 sm:grid-cols-2">
                <Input
                  label="Full name"
                  autoComplete="name"
                  error={profileErrors.fullName?.message}
                  {...profileForm.register('fullName')}
                />
                <Input
                  label="Phone number"
                  type="tel"
                  autoComplete="tel"
                  error={profileErrors.phoneNumber?.message}
                  {...profileForm.register('phoneNumber')}
                />
              </div>
              <div className="grid gap-5 sm:grid-cols-2">
                <ReadOnlyField label="Email" value={profile.user.email} badge="read-only" />
                <ReadOnlyField label="Roll number" value={profile.roll_number} badge="read-only" />
              </div>
              <p className="text-xs leading-relaxed text-ink-400">
                Roll Number is your permanent student identity and cannot be changed. For corrections, please contact
                your Learner Support Centre.
              </p>

              {saveState === 'saved' && (
                <div className="rounded-xl border border-emerald-200 bg-emerald-50 px-4 py-3 text-sm text-emerald-800" role="status">
                  Profile updated successfully.
                </div>
              )}
              {saveError && (
                <div className="rounded-xl border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-800" role="alert">
                  {saveError}
                </div>
              )}

              <div className="flex items-center gap-3">
                <Button type="submit" loading={profileForm.formState.isSubmitting} disabled={!profileForm.formState.isDirty}>
                  Save changes
                </Button>
                {profileForm.formState.isDirty && saveState === 'saved' && (
                  <span className="text-xs text-ink-400">You have unsaved edits</span>
                )}
              </div>
            </form>
          </Card>

          {/* Academic information — institution-controlled */}
          <Card variant="solid" padding="md">
            <SectionTitle label="Academic information" dot="bg-teal-500" />
            <div className="mt-6 grid gap-5 sm:grid-cols-3">
              <ReadOnlyField label="Program" value={profile.program?.name ?? ''} badge="read-only" />
              <ReadOnlyField label="Current semester" value={profile.semester?.name ?? ''} badge="read-only" />
              <ReadOnlyField label="LSC" value={profile.lsc?.name ?? ''} badge="read-only" />
            </div>
            <div className="mt-5 rounded-xl border border-dashed border-slate-200 bg-slate-50/60 px-4 py-3">
              <p className="text-xs font-medium text-ink-700">Set by the institution</p>
              <p className="mt-1 text-xs leading-relaxed text-ink-500">
                Program, Semester and LSC are assigned at admission and maintained by the institution. If something
                looks wrong, please contact your Learner Support Centre.
              </p>
            </div>
          </Card>

          {/* Security */}
          <Card variant="solid" padding="md">
            <SectionTitle label="Security" dot="bg-rose-400" />

            {/* Change password */}
            <form onSubmit={passwordForm.handleSubmit(onChangePassword)} noValidate className="mt-6">
              <h3 className="text-sm font-semibold text-ink-900">Change password</h3>
              <p className="mt-1 text-sm text-ink-500">
                Choose a strong password you don&apos;t use anywhere else. Changing it signs out your other sessions.
              </p>
              <div className="mt-4 grid gap-5 sm:grid-cols-2">
                <div className="sm:col-span-2">
                  <Input
                    label="Current password"
                    type="password"
                    autoComplete="current-password"
                    error={passwordErrors.currentPassword?.message}
                    {...passwordForm.register('currentPassword')}
                  />
                </div>
                <Input
                  label="New password"
                  type="password"
                  autoComplete="new-password"
                  hint="At least 8 characters, with a letter and a number"
                  error={passwordErrors.newPassword?.message}
                  {...passwordForm.register('newPassword')}
                />
                <Input
                  label="Confirm new password"
                  type="password"
                  autoComplete="new-password"
                  error={passwordErrors.confirmPassword?.message}
                  {...passwordForm.register('confirmPassword')}
                />
              </div>

              {pwState === 'saved' && (
                <div className="mt-4 rounded-xl border border-emerald-200 bg-emerald-50 px-4 py-3 text-sm text-emerald-800" role="status">
                  Password changed successfully. Your other sessions have been signed out.
                </div>
              )}
              {pwError && (
                <div className="mt-4 rounded-xl border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-800" role="alert">
                  {pwError}
                </div>
              )}

              <Button type="submit" variant="secondary" className="mt-4" loading={passwordForm.formState.isSubmitting}>
                Update password
              </Button>
            </form>

            {/* Active sessions */}
            <div className="mt-8 border-t border-slate-100 pt-8">
              <div className="flex flex-wrap items-center justify-between gap-3">
                <div>
                  <h3 className="text-sm font-semibold text-ink-900">Active sessions</h3>
                  <p className="mt-1 text-sm text-ink-500">
                    Devices currently signed in to your account. You can sign out any of them remotely.
                  </p>
                </div>
                {sessionsError && (
                  <button onClick={loadSessions} className="text-sm font-medium text-dg-700 hover:text-dg-900 focus-ring rounded">
                    Retry
                  </button>
                )}
              </div>

              {sessionsLoading ? (
                <div className="mt-4 space-y-3">
                  <Skeleton className="h-14" />
                  <Skeleton className="h-14" />
                  <Skeleton className="h-14" />
                </div>
              ) : sessionsError ? (
                <div className="mt-4 rounded-xl border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-800" role="alert">
                  {sessionsError}
                </div>
              ) : (
                <ul className="mt-2 divide-y divide-slate-100">
                  {(sessions ?? []).map((s) => (
                    <li key={s.id} className="flex items-start justify-between gap-4 py-4">
                      <div className="min-w-0">
                        <p className="flex flex-wrap items-center gap-2 text-sm font-medium text-ink-900">
                          {describeDevice(s.deviceInfo)}
                          {s.isCurrent && (
                            <span className="rounded-full bg-emerald-50 px-2 py-0.5 text-[11px] font-medium text-emerald-700 ring-1 ring-emerald-200">
                              This device
                            </span>
                          )}
                          {!s.isActive && (
                            <span className="rounded-full bg-slate-100 px-2 py-0.5 text-[11px] font-medium text-ink-500">
                              Signed out
                            </span>
                          )}
                        </p>
                        <p className="mt-1 text-xs text-ink-500">
                          IP {s.ipAddress ?? 'unknown'} · Signed in {fmtDateTime(s.createdAt)}
                        </p>
                      </div>
                      {s.isActive && !s.isCurrent && (
                        <Button
                          variant="ghost"
                          size="sm"
                          className="shrink-0 text-rose-600 hover:bg-rose-50 hover:text-rose-700"
                          loading={revokingId === s.id}
                          onClick={() => revokeSession(s.id)}
                        >
                          Sign out
                        </Button>
                      )}
                    </li>
                  ))}
                </ul>
              )}
            </div>

            {/* Logout */}
            <div className="mt-8 border-t border-slate-100 pt-8">
              <h3 className="text-sm font-semibold text-ink-900">Log out</h3>
              <p className="mt-1 text-sm text-ink-500">
                Ends this session on the server, so the sign-in can&apos;t be reused on this device.
              </p>
              <Button variant="secondary" className="mt-4" onClick={handleLogout} loading={isLoggingOut}>
                Log out
              </Button>
            </div>
          </Card>
        </div>

        {/* ── Secondary column ────────────────────────────────────────────── */}
        <div className="space-y-6">
          {/* Student identity */}
          <Card variant="solid" padding="md">
            <div className="flex items-center gap-4">
              <span
                className="flex h-16 w-16 shrink-0 items-center justify-center rounded-full bg-dg-900 text-lg font-semibold text-white shadow-sm"
                aria-hidden
              >
                {initials(profile.full_name)}
              </span>
              <div className="min-w-0">
                <p className="truncate text-base font-semibold text-ink-900">{profile.full_name}</p>
                <p className="mt-0.5 text-sm text-ink-500">Roll No. {profile.roll_number}</p>
                <div className="mt-2">
                  <StatusBadge status={status} />
                </div>
              </div>
            </div>
          </Card>

          {/* Account information */}
          <Card variant="solid" padding="md">
            <SectionTitle label="Account information" dot="bg-slate-300" />
            <div className="mt-3 divide-y divide-slate-100">
              <AccountInfoRow label="Member since">{fmtDate(profile.user.created_at)}</AccountInfoRow>
              <AccountInfoRow label="Last login">{fmtDateTime(profile.user.last_login_at)}</AccountInfoRow>
              <AccountInfoRow label="Account status">
                <StatusBadge status={status} />
              </AccountInfoRow>
              <AccountInfoRow label="Active sessions">{activeCount ?? '—'}</AccountInfoRow>
              <AccountInfoRow label="This session">
                {currentSession ? describeDevice(currentSession.deviceInfo) : '—'}
              </AccountInfoRow>
            </div>
          </Card>

          {/* Preferences — only basics are supported today; no AI-learning
              preferences exist in this phase, so nothing is invented here. */}
          <Card variant="subtle" padding="md">
            <h3 className="text-sm font-semibold text-ink-900">Preferences</h3>
            <p className="mt-1 text-sm leading-relaxed text-ink-500">
              Notification and display preferences will appear here once supported. No other preference data is stored
              about you.
            </p>
            <p className="mt-3 inline-flex rounded-full bg-slate-100 px-2.5 py-1 text-[11px] font-medium text-ink-500">
              Coming in a later phase
            </p>
          </Card>
        </div>
      </div>
    </div>
  );
}
