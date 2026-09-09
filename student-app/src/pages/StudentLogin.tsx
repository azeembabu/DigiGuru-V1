import * as React from 'react';
import { useForm } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';
import { Link, useLocation, useNavigate } from 'react-router-dom';
import { loginSchema, type LoginInput } from '../lib/validations';
import { api, persistTokens } from '../lib/api';
import { useAuthStore } from '../store/authStore';
import { Button } from '../components/ui/Button';
import { Input } from '../components/ui/Input';

export default function StudentLogin() {
  const navigate = useNavigate();
  const location = useLocation() as { state?: { from?: { pathname: string } } };
  const setUser = useAuthStore((s) => s.setUser);
  const [serverError, setServerError] = React.useState<string | null>(null);
  const [showPassword, setShowPassword] = React.useState(false);

  const {
    register,
    handleSubmit,
    formState: { errors, isSubmitting },
  } = useForm<LoginInput>({
    resolver: zodResolver(loginSchema),
    defaultValues: { identifier: '', password: '', rememberMe: false },
  });

  async function onSubmit(values: LoginInput) {
    setServerError(null);
    try {
      const { data: envelope } = await api.post('/auth/student/login', {
        identifier: values.identifier.trim(),
        password: values.password,
        rememberMe: values.rememberMe,
      });

      // Backend response envelope: { success, data: { accessToken, refreshToken, user, student } }
      const data = envelope?.data ?? envelope;
      const accessToken: string = data.accessToken ?? data.access_token ?? data.token;
      const refreshToken: string = data.refreshToken ?? data.refresh_token ?? '';
      const user = data.user ?? data.student ?? null;

      if (!accessToken) throw new Error('Missing access token in response');

      persistTokens({ accessToken, refreshToken }, !!values.rememberMe);

      if (user) {
        const normalized = {
          id: String(user.id ?? user._id ?? user.rollNumber ?? ''),
          fullName: user.fullName ?? user.name ?? '',
          rollNumber: user.rollNumber ?? user.roll_number ?? values.identifier,
          email: user.email ?? '',
          phoneNumber: user.phoneNumber ?? user.phone_number ?? user.phone ?? '',
          program: user.program ?? '',
          semester: user.semester ?? user.sem ?? '',
          lsc: user.lsc ?? '',
          role: 'STUDENT' as const,
        };
        setUser(normalized);
      } else {
        // Fetch profile if login did not return user
        try {
          const { data: me } = await api.get('/auth/me');
          const u = me.user ?? me;
          setUser({
            id: String(u.id ?? u._id ?? ''),
            fullName: u.fullName ?? u.name ?? '',
            rollNumber: u.rollNumber ?? '',
            email: u.email ?? '',
            phoneNumber: u.phoneNumber ?? u.phone ?? '',
            program: u.program ?? '',
            semester: u.semester ?? '',
            lsc: u.lsc ?? '',
            role: 'STUDENT',
          });
        } catch {
          setUser(null);
        }
      }

      const dest = location.state?.from?.pathname ?? '/student/dashboard';
      navigate(dest, { replace: true });
    } catch (err: unknown) {
      const msg =
        (err as { response?: { data?: { message?: string; error?: string } } })?.response?.data?.message ??
        (err as { response?: { data?: { message?: string; error?: string } } })?.response?.data?.error ??
        (err as Error)?.message ??
        'Login failed. Please check your credentials and try again.';
      setServerError(msg);
    }
  }

  return (
    <div className="relative flex min-h-screen">
      {/* Full background image */}
      <img
        src="/nature-bg.jpg"
        alt=""
        className="absolute inset-0 h-full w-full object-cover"
      />

      {/* Right side overlay with translucency */}
      <div className="absolute inset-0 lg:inset-0 lg:left-1/2 lg:bg-white/20 lg:backdrop-blur-sm" />

      {/* Content container */}
      <div className="relative z-10 flex w-full flex-col lg:w-1/2 lg:ml-auto">
        {/* Header */}
        <header className="flex items-center justify-between px-6 py-5 sm:px-10">
          <Link to="/login" className="flex items-center gap-2.5 focus-ring rounded-xl">
            <span className="flex h-8 w-8 items-center justify-center rounded-xl bg-dg-900 text-white text-[15px] font-bold shadow-sm" aria-hidden>
              ◆
            </span>
            <span className="text-[17px] font-semibold tracking-tight text-ink-900">Digi Guru</span>
          </Link>
          <Link
            to="/signup"
            className="inline-flex items-center justify-center rounded-full bg-dg-900 px-5 py-2.5 text-sm font-medium text-white shadow-[0_4px_16px_rgba(14,75,58,0.22)] transition hover:bg-dg-950 focus-ring"
          >
            Get Started
          </Link>
        </header>

        {/* Login form container */}
        <div className="flex flex-1 flex-col justify-center px-6 py-8 sm:px-10">
          <div className="mx-auto w-full max-w-[420px]">
            {/* Heading */}
            <div className="mb-8">
              <p className="font-display text-[11px] font-medium uppercase tracking-[0.18em] text-dg-700/80">Welcome back</p>
              <h1 className="mt-2 font-display text-[28px] font-normal leading-tight text-ink-900 sm:text-[34px]">
                Learn with your <span className="font-medium text-dg-900">personal AI teacher</span>
              </h1>
              <p className="mt-2 text-sm leading-relaxed text-ink-500 sm:text-[15px]">
                Your textbook, your pace — calm, focused, and built for you.
              </p>
            </div>

            {/* Form card */}
            <div className="rounded-2xl border border-slate-200/50 bg-white/90 backdrop-blur-sm p-6 shadow-lg sm:p-8">
              <div className="mb-6">
                <h2 className="text-xl font-semibold tracking-tight text-ink-900">Log in to your account</h2>
                <p className="mt-1 text-sm text-ink-500">Use your Roll Number or Email to continue.</p>
              </div>

              {serverError && (
                <div className="mb-5 rounded-xl border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-800" role="alert">
                  {serverError}
                </div>
              )}

              <form onSubmit={handleSubmit(onSubmit)} noValidate className="space-y-4">
                <Input
                  label="Roll Number / Email"
                  placeholder="012345 or you@example.com"
                  autoComplete="username"
                  required
                  error={errors.identifier?.message}
                  {...register('identifier')}
                />

                <div className="space-y-1.5">
                  <label htmlFor="password" className="block text-sm font-medium text-ink-900">
                    Password <span className="ml-1 text-red-500" aria-hidden>*</span>
                  </label>
                  <div className="relative">
                    <input
                      id="password"
                      type={showPassword ? 'text' : 'password'}
                      placeholder="••••••••"
                      autoComplete="current-password"
                      aria-invalid={!!errors.password}
                      aria-describedby={errors.password ? 'password-error' : undefined}
                      className={[
                        'block w-full rounded-xl border bg-white px-3.5 py-2.5 pr-10 text-[15px] text-ink-900 placeholder:text-ink-400',
                        'shadow-sm transition-colors focus:outline-none focus:ring-2 focus:ring-dg-500/20 focus:border-dg-500',
                        errors.password ? 'border-red-300 focus:border-red-400 focus:ring-red-500/20' : 'border-slate-200 hover:border-slate-300',
                      ].join(' ')}
                      {...register('password')}
                    />
                    <button
                      type="button"
                      onClick={() => setShowPassword((v) => !v)}
                      className="absolute inset-y-0 right-2 flex items-center rounded-lg px-2 text-ink-400 hover:text-ink-700 focus-ring"
                      aria-label={showPassword ? 'Hide password' : 'Show password'}
                    >
                      <span className="text-xs font-medium">{showPassword ? 'Hide' : 'Show'}</span>
                    </button>
                  </div>
                  {errors.password?.message && (
                    <p id="password-error" className="text-sm text-red-600" role="alert">
                      {errors.password.message}
                    </p>
                  )}
                </div>

                <div className="flex items-center justify-between gap-4">
                  <label className="flex cursor-pointer items-center gap-2 text-sm text-ink-700">
                    <input
                      type="checkbox"
                      className="h-4 w-4 rounded border-slate-300 text-dg-600 focus:ring-dg-500"
                      {...register('rememberMe')}
                    />
                    Remember me
                  </label>
                  <Link to="/forgot-password" className="text-sm font-medium text-dg-700 hover:text-dg-900 hover:underline focus-ring rounded">
                    Forgot password?
                  </Link>
                </div>

                <Button type="submit" loading={isSubmitting} className="w-full" size="lg">
                  Log in
                </Button>

                <p className="text-center text-sm text-ink-500">
                  Don&apos;t have an account?{' '}
                  <Link to="/signup" className="font-medium text-dg-700 hover:text-dg-900 hover:underline focus-ring rounded">
                    Sign up
                  </Link>
                </p>
              </form>
            </div>

            <p className="mt-6 flex items-center justify-center gap-2 text-center text-xs text-ink-400 sm:text-sm">
              <span className="inline-flex h-5 w-5 items-center justify-center rounded-full bg-white/80 text-[11px] shadow-sm">♥</span>
              Trusted by learners across Kerala — your data stays private and secure.
            </p>
          </div>
        </div>
      </div>
    </div>
  );
}
