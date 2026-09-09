import * as React from 'react';
import { useForm } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';
import { Link, useNavigate, useSearchParams } from 'react-router-dom';
import { resetPasswordSchema, type ResetPasswordInput } from '../lib/validations';
import { api } from '../lib/api';
import { Button } from '../components/ui/Button';
import { Input } from '../components/ui/Input';

export default function ResetPassword() {
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();
  const token = searchParams.get('token') ?? '';
  const [serverError, setServerError] = React.useState<string | null>(null);
  const [done, setDone] = React.useState(false);

  const {
    register,
    handleSubmit,
    formState: { errors, isSubmitting },
  } = useForm<ResetPasswordInput>({
    resolver: zodResolver(resetPasswordSchema),
    defaultValues: { password: '', confirmPassword: '' },
  });

  async function onSubmit(values: ResetPasswordInput) {
    setServerError(null);
    if (!token) {
      setServerError('Reset link is missing or expired. Please request a new link.');
      return;
    }
    try {
      await api.post('/auth/student/reset-password', {
        token,
        password: values.password,
      });
      setDone(true);
      setTimeout(() => navigate('/login', { replace: true }), 1800);
    } catch (err: unknown) {
      const msg =
        (err as { response?: { data?: { message?: string; error?: string } } })?.response?.data?.message ??
        (err as { response?: { data?: { message?: string; error?: string } } })?.response?.data?.error ??
        'Reset failed. The link may have expired.';
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
          <Link to="/login" className="inline-flex rounded-full border border-slate-200 bg-white/90 px-5 py-2.5 text-sm font-medium text-ink-700 transition hover:bg-white focus-ring">
            Back to login
          </Link>
        </header>

        {/* Form container */}
        <div className="flex flex-1 flex-col justify-center px-6 py-8 sm:px-10">
          <div className="mx-auto w-full max-w-[420px]">
            {/* Form card */}
            <div className="rounded-2xl border border-slate-200/50 bg-white/90 backdrop-blur-sm p-6 shadow-lg sm:p-8">
              <h1 className="text-xl font-semibold tracking-tight text-ink-900">Reset password</h1>
              <p className="mt-1 text-sm text-ink-500">Choose a new password for your account.</p>

              {!token && (
                <div className="mt-4 rounded-xl border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-900">
                  This reset link is invalid or has expired.{' '}
                  <Link to="/forgot-password" className="font-medium underline">
                    Request a new link
                  </Link>
                  .
                </div>
              )}

              {done ? (
                <div className="mt-6 rounded-xl border border-emerald-200 bg-emerald-50 px-4 py-4">
                  <p className="text-sm font-medium text-emerald-900">Password updated</p>
                  <p className="mt-1 text-sm text-emerald-800">Redirecting you to login…</p>
                  <Link to="/login" className="mt-3 inline-flex text-sm font-medium text-dg-700 hover:underline">
                    Go to login now
                  </Link>
                </div>
              ) : (
                <form onSubmit={handleSubmit(onSubmit)} noValidate className="mt-6 space-y-4">
                  {serverError && (
                    <div className="rounded-xl border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-800" role="alert">
                      {serverError}
                    </div>
                  )}
                  <Input label="New password" type="password" placeholder="At least 8 characters" autoComplete="new-password" required error={errors.password?.message} {...register('password')} />
                  <Input label="Confirm new password" type="password" placeholder="Repeat password" autoComplete="new-password" required error={errors.confirmPassword?.message} {...register('confirmPassword')} />
                  <Button type="submit" loading={isSubmitting} className="w-full" size="lg">
                    Reset password
                  </Button>
                  <p className="text-center text-sm text-ink-500">
                    <Link to="/login" className="font-medium text-dg-700 hover:text-dg-900 hover:underline focus-ring rounded">
                      Back to login
                    </Link>
                  </p>
                </form>
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
