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
        aria-hidden="true"
      />

      {/* Mobile-only header */}
      <div className="relative z-10 flex items-center gap-2 px-5 py-4 md:hidden">
        <div className="flex h-9 w-9 items-center justify-center rounded-xl bg-white/90 shadow-sm">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path
              d="M12 3L20 9V21H4V9L12 3Z"
              stroke="#0B4D3B"
              strokeWidth="2.2"
              strokeLinejoin="round"
            />
            <path d="M9 21V15H15V21" stroke="#0B4D3B" strokeWidth="2.2" strokeLinejoin="round" />
          </svg>
        </div>
        <span className="text-base font-semibold text-white drop-shadow">Digi Guru</span>
      </div>

      {/* Left brand panel */}
      <div className="relative hidden w-full md:flex md:w-1/2 md:flex-col md:justify-center md:px-16 lg:px-20">
        <div className="max-w-md">
          <div className="mb-8 flex items-center gap-3">
            <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-white/15 backdrop-blur">
              <svg width="22" height="22" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                <path
                  d="M12 3L20 9V21H4V9L12 3Z"
                  stroke="white"
                  strokeWidth="2"
                  strokeLinejoin="round"
                />
                <path d="M9 21V15H15V21" stroke="white" strokeWidth="2" strokeLinejoin="round" />
              </svg>
            </div>
            <span className="text-xl font-bold text-white">Digi Guru</span>
          </div>

          <h1 className="mb-4 text-4xl font-bold leading-[1.1] tracking-tight text-white">
            Your learning journey starts here
          </h1>

          <p className="mb-10 max-w-sm text-lg leading-relaxed text-white/85">
            Continue your personalized learning experience with Digi Guru.
          </p>

          <div className="flex items-end gap-4" aria-hidden="true">
            <div className="flex h-[88px] w-[88px] items-center justify-center rounded-[16px] bg-white/10 backdrop-blur">
              <svg width="36" height="36" viewBox="0 0 24 24" fill="none">
                <path
                  d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20"
                  stroke="white"
                  strokeWidth="2"
                  strokeLinecap="round"
                />
                <path
                  d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z"
                  stroke="white"
                  strokeWidth="2"
                  strokeLinejoin="round"
                />
              </svg>
            </div>

            <div className="flex flex-col gap-[6px] pb-[10px]">
              {[7, 6, 5].map((size) => (
                <div
                  key={size}
                  className="rounded-full bg-white/75"
                  style={{ width: size, height: size }}
                />
              ))}
            </div>

            <div className="flex h-[64px] w-[64px] items-center justify-center rounded-[16px] bg-white/10 backdrop-blur">
              <svg width="28" height="28" viewBox="0 0 24 24" fill="none">
                <path
                  d="M12 14l9-5-9-5-9 5 9 5z"
                  stroke="white"
                  strokeWidth="2"
                  strokeLinejoin="round"
                />
                <path
                  d="M12 14l6.16-3.402a12.083 12.083 0 0 1 .665 6.479A11.952 11.952 0 0 1 12 20.055a11.952 11.952 0 0 1-6.824-2.096A12.083 12.083 0 0 1 5.84 10.598L12 14z"
                  stroke="white"
                  strokeWidth="2"
                  strokeLinejoin="round"
                />
              </svg>
            </div>

            <div className="ml-auto flex items-end gap-[8px]" aria-hidden="true">
              {[
                { w: 22, h: 36 },
                { w: 18, h: 52 },
                { w: 14, h: 28 },
              ].map(({ w, h }, idx) => (
                <div
                  key={idx}
                  className="rounded-[4px] bg-white/10"
                  style={{ width: w, height: h }}
                />
              ))}
            </div>
          </div>
        </div>
      </div>

      {/* Right login card */}
      <div className="relative flex w-full items-center justify-center px-4 py-8 md:w-1/2 md:px-10">
        <div className="w-full max-w-[440px] rounded-[24px] bg-white p-[48px] shadow-sm">
          <div className="mb-8 text-center">
            <span className="mb-2 block text-xs font-semibold uppercase tracking-widest text-[#1A8A68]">
              Student Portal
            </span>
            <h2 className="mb-1 text-[28px] font-bold tracking-tight text-[#0f172a]">
              Student Login
            </h2>
            <p className="text-sm text-[#64748b]">Sign in to continue your learning journey.</p>
          </div>

          {serverError && (
            <div
              className="mb-4 flex items-start gap-2 rounded-[12px] border border-red-200 bg-red-50 p-3"
              role="alert"
              aria-live="polite"
            >
              <svg
                className="mt-[2px] h-[18px] w-[18px] shrink-0 text-red-600"
                viewBox="0 0 24 24"
                fill="none"
                aria-hidden="true"
              >
                <path
                  d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"
                  stroke="currentColor"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
                <line x1="12" y1="9" x2="12" y2="13" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
                <line x1="12" y1="17" x2="12.01" y2="17" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
              </svg>
              <span className="text-sm font-medium leading-relaxed text-red-600">{serverError}</span>
            </div>
          )}

          <form onSubmit={handleSubmit(onSubmit)} noValidate className="flex flex-col gap-[20px]">
            <div>
              <label htmlFor="identifier" className="mb-1 block text-sm font-semibold text-[#334155]">
                Roll Number <span className="text-red-600">*</span>
              </label>
              <Input
                id="identifier"
                type="text"
                inputMode="text"
                autoComplete="username"
                placeholder="Enter your roll number"
                error={errors.identifier?.message}
                {...register('identifier')}
              />
            </div>

            <div>
              <label htmlFor="password" className="mb-1 block text-sm font-semibold text-[#334155]">
                Password <span className="text-red-600">*</span>
              </label>
              <div className="relative">
                <Input
                  id="password"
                  type={showPassword ? 'text' : 'password'}
                  autoComplete="current-password"
                  placeholder="Enter your password"
                  error={errors.password?.message}
                  {...register('password')}
                />
                <button
                  type="button"
                  onClick={() => setShowPassword((v) => !v)}
                  className="absolute right-[10px] top-1/2 -translate-y-1/2 rounded-md px-[10px] py-1 text-xs font-medium text-[#94a3b8] transition-colors hover:bg-[#F8FAFC] hover:text-[#334155] focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#0F6B52] disabled:opacity-45 disabled:cursor-not-allowed"
                  disabled={isSubmitting}
                  aria-label={showPassword ? 'Hide password' : 'Show password'}
                >
                  {showPassword ? (
                    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                      <path
                        d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7.36 0-13.91-5.55-18-13a9.956 9.956 0 0 1 6.06-6.06M9.9 4.24A9.884 9.884 0 0 1 12 4c7.36 0 13.91 5.55 18 13-1.27 4.06-5.06 7-9.54 7-1.93 0-3.68-.56-5.2-1.52"
                        stroke="currentColor"
                        strokeWidth="1.8"
                        strokeLinecap="round"
                        strokeLinejoin="round"
                      />
                      <line x1="1" y1="1" x2="23" y2="23" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
                    </svg>
                  ) : (
                    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                      <path
                        d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7-10-7-10-7z"
                        stroke="currentColor"
                        strokeWidth="1.8"
                        strokeLinecap="round"
                        strokeLinejoin="round"
                      />
                      <circle cx="12" cy="12" r="3" stroke="currentColor" strokeWidth="1.8" />
                    </svg>
                  )}
                </button>
              </div>
            </div>

            <div className="flex items-center justify-between gap-4">
              <label className="flex cursor-pointer items-center gap-2">
                <input
                  type="checkbox"
                  className="h-[18px] w-[18px] rounded-[5px] border-[1.5px] border-[#e2e8f0] accent-[#0F6B52]"
                  {...register('rememberMe')}
                />
                <span className="text-sm text-[#334155] select-none">Keep me signed in on this device</span>
              </label>
              <Link
                to="/forgot-password"
                className="whitespace-nowrap text-sm font-medium text-[#0F6B52] no-underline transition-colors hover:text-[#0B4D3B] hover:underline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#0F6B52]"
              >
                Forgot Password?
              </Link>
            </div>

            <Button type="submit" className="mt-1 h-[56px] w-full" disabled={isSubmitting}>
              {isSubmitting ? 'Signing in…' : 'Login'}
            </Button>
          </form>

          <div className="mx-auto mt-[24px] flex w-fit items-center gap-3 border-t border-t-[#e2e8f0] pt-4">
            <span className="text-sm text-[#64748b]">New student?</span>
            <Link
              to="/signup"
              className="whitespace-nowrap text-sm font-semibold text-[#0F6B52] no-underline transition-colors hover:text-[#0B4D3B] hover:underline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#0F6B52]"
            >
              Create Student Account
            </Link>
          </div>

          <p className="mx-auto mt-[24px] flex items-center justify-center gap-2 text-center text-xs text-[#94a3b8]">
            <span
              className="flex h-[22px] w-[22px] items-center justify-center rounded-full bg-white shadow-sm"
              aria-hidden="true"
            >
              <svg width="11" height="11" viewBox="0 0 24 24" fill="none">
                <path
                  d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z"
                  fill="#0B4D3B"
                />
              </svg>
            </span>
            Trusted by learners across Kerala — your data stays private and secure.
          </p>
        </div>
      </div>
    </div>
  );
}
