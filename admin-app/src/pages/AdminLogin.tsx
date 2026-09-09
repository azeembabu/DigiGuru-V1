/**
 * AdminLogin — dark/professional, no signup link.
 * Admins are created via backend/seed. Calls POST /api/auth/admin/login.
 */

import * as React from 'react';
import { useForm } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';
import { useNavigate, useLocation, Link } from 'react-router-dom';
import { api, getErrorMessage, isForbiddenError } from '@/lib/api';
import { adminLoginSchema, type AdminLoginInput } from '@/lib/validations';
import { useAuthStore } from '@/store/authStore';
import { Button } from '@/components/ui/Button';
import { Input } from '@/components/ui/Input';

interface LoginResponse {
  accessToken: string;
  user: { id: string; email: string; fullName: string; role: 'ADMIN' };
}

export function AdminLogin() {
  const navigate = useNavigate();
  const location = useLocation();
  const { isAuthenticated, setSession } = useAuthStore();
  const [serverError, setServerError] = React.useState<string | null>(null);

  const from = (location.state as { from?: { pathname: string } } | null)?.from?.pathname ?? '/admin/dashboard';

  React.useEffect(() => {
    if (isAuthenticated) navigate(from, { replace: true });
  }, [isAuthenticated, navigate, from]);

  const {
    register,
    handleSubmit,
    formState: { errors, isSubmitting },
  } = useForm<AdminLoginInput>({
    resolver: zodResolver(adminLoginSchema),
    defaultValues: { identifier: '', password: '' },
  });

  const onSubmit = async (values: AdminLoginInput) => {
    setServerError(null);
    try {
      const res = await api.post<{ success: true; data: LoginResponse }>('/auth/admin/login', {
        email: values.identifier.includes('@') ? values.identifier.trim() : undefined,
        identifier: values.identifier.trim(),
        password: values.password,
      });
      const { accessToken, user } = res.data.data;
      if (user.role !== 'ADMIN') {
        setServerError('Access Denied — Admin only. This account does not have admin privileges.');
        return;
      }
      setSession(accessToken, user);
      navigate(from, { replace: true });
    } catch (err: unknown) {
      if (isForbiddenError(err)) {
        setServerError('Access Denied — Admin only. Your account does not have admin privileges.');
        return;
      }
      setServerError(getErrorMessage(err));
    }
  };

  return (
    <div className="min-h-screen bg-admin-950">
      {/* Subtle grid + gradient backdrop */}
      <div className="pointer-events-none absolute inset-0 bg-[linear-gradient(to_right,rgba(255,255,255,0.04)_1px,transparent_1px),linear-gradient(to_bottom,rgba(255,255,255,0.04)_1px,transparent_1px)] bg-[size:32px_32px]" />
      <div className="pointer-events-none absolute inset-0 bg-gradient-to-b from-indigo-900/30 via-transparent to-admin-950" />

      <div className="relative flex min-h-screen">
        {/* Left — brand / context */}
        <div className="hidden flex-1 flex-col justify-between p-10 lg:flex">
          <div>
            <Link to="/admin/login" className="inline-flex items-center gap-3">
              <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-indigo-600 text-white shadow-lg">
                <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden>
                  <path d="M12 2L2 7l10 5 10-5-10-5z" />
                  <path d="M2 17l10 5 10-5" />
                  <path d="M2 12l10 5 10-5" />
                </svg>
              </div>
              <span className="text-lg font-semibold tracking-tight text-white">Digi Guru</span>
              <span className="rounded-full border border-white/20 bg-white/10 px-2.5 py-1 text-xs font-medium tracking-widest text-white/80">
                ADMIN
              </span>
            </Link>

            <div className="mt-16 max-w-md">
              <h1 className="text-3xl font-bold tracking-tight text-white">
                Admin Portal
                <span className="block text-indigo-300">Digi Guru</span>
              </h1>
              <p className="mt-4 text-sm leading-6 text-slate-300">
                Manage students, programs, semesters, and Learner Support Centres. Access is restricted and all actions are audited.
              </p>
              <ul className="mt-8 space-y-3 text-sm text-slate-400">
                <li className="flex gap-2">
                  <span className="mt-1 h-1.5 w-1.5 shrink-0 rounded-full bg-indigo-400" />
                  Separate from the Student app — no shared session.
                </li>
                <li className="flex gap-2">
                  <span className="mt-1 h-1.5 w-1.5 shrink-0 rounded-full bg-indigo-400" />
                  Admin accounts are created by the backend — no public signup.
                </li>
                <li className="flex gap-2">
                  <span className="mt-1 h-1.5 w-1.5 shrink-0 rounded-full bg-indigo-400" />
                  Student credentials cannot access this portal (403).
                </li>
              </ul>
            </div>
          </div>

          <p className="text-xs text-slate-500">
            Digi Guru — Online Tuition Centre. Admin access is role-gated at the API layer.
          </p>
        </div>

        {/* Right — form card */}
        <div className="flex flex-1 items-center justify-center p-6 lg:p-10">
          <div className="w-full max-w-[420px]">
            {/* Mobile brand */}
            <div className="mb-8 lg:hidden">
              <div className="flex items-center gap-3">
                <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-indigo-600 text-white">
                  <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden>
                    <path d="M12 2L2 7l10 5 10-5-10-5z" />
                    <path d="M2 17l10 5 10-5" />
                    <path d="M2 12l10 5 10-5" />
                  </svg>
                </div>
                <span className="text-lg font-semibold text-white">Digi Guru</span>
                <span className="rounded-full border border-white/20 bg-white/10 px-2.5 py-1 text-xs font-medium tracking-widest text-white/80">
                  ADMIN
                </span>
              </div>
              <h1 className="mt-6 text-2xl font-bold tracking-tight text-white">Admin Portal — Digi Guru</h1>
              <p className="mt-2 text-sm text-slate-400">Sign in with your administrator credentials.</p>
            </div>

            <div className="rounded-2xl border border-white/10 bg-white/20 p-6 shadow-2xl backdrop-blur-sm sm:p-8">
              <div className="hidden lg:block">
                <h2 className="text-lg font-semibold tracking-tight text-admin-900">Sign in</h2>
                <p className="mt-1 text-sm text-admin-500">Admin ID / Email and password.</p>
              </div>

              {serverError && (
                <div
                  role="alert"
                  className="mt-4 rounded-lg border border-red-200 bg-red-50 px-4 py-3 text-sm font-medium text-red-800 lg:mt-6"
                >
                  {serverError}
                </div>
              )}

              <form onSubmit={handleSubmit(onSubmit)} className="mt-6 space-y-4" noValidate>
                <Input
                  label="Admin ID / Email"
                  placeholder="admin@digiguru.com or ADMIN-001"
                  autoComplete="username"
                  autoFocus
                  error={errors.identifier?.message}
                  {...register('identifier')}
                />

                <Input
                  label="Password"
                  type="password"
                  placeholder="••••••••"
                  autoComplete="current-password"
                  error={errors.password?.message}
                  {...register('password')}
                />

                <Button type="submit" loading={isSubmitting} className="mt-2 w-full" size="lg">
                  {isSubmitting ? 'Signing in…' : 'Login'}
                </Button>

                <p className="text-center text-xs leading-5 text-admin-500">
                  No account? Contact the system administrator. Accounts are provisioned via the backend.
                </p>
              </form>

              <div className="mt-6 rounded-lg bg-admin-50 px-4 py-3">
                <p className="text-xs font-medium text-admin-700">Security notice</p>
                <p className="mt-1 text-xs leading-5 text-admin-500">
                  Student credentials cannot access this portal. A STUDENT token hitting <code className="rounded bg-white px-1 py-0.5 font-mono text-[11px]">/api/admin/*</code> receives <code className="font-mono text-[11px]">403 Forbidden</code>.
                </p>
              </div>
            </div>

            <p className="mt-6 text-center text-xs text-slate-500">
              This is a separate application from the Student portal and runs on a different domain/port.
            </p>
          </div>
        </div>
      </div>
    </div>
  );
}
