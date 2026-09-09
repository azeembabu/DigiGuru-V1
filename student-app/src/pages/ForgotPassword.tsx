import * as React from 'react';
import { useForm } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';
import { Link } from 'react-router-dom';
import { forgotPasswordSchema, type ForgotPasswordInput } from '../lib/validations';
import { api } from '../lib/api';
import { Button } from '../components/ui/Button';
import { Input } from '../components/ui/Input';

export default function ForgotPassword() {
  const [serverError, setServerError] = React.useState<string | null>(null);
  const [success, setSuccess] = React.useState(false);

  const {
    register,
    handleSubmit,
    formState: { errors, isSubmitting },
  } = useForm<ForgotPasswordInput>({
    resolver: zodResolver(forgotPasswordSchema),
    defaultValues: { email: '' },
  });

  async function onSubmit(values: ForgotPasswordInput) {
    setServerError(null);
    setSuccess(false);
    try {
      await api.post('/auth/student/forgot-password', { email: values.email.trim().toLowerCase() });
      setSuccess(true);
    } catch (err: unknown) {
      const msg =
        (err as { response?: { data?: { message?: string; error?: string } } })?.response?.data?.message ??
        (err as { response?: { data?: { message?: string; error?: string } } })?.response?.data?.error ??
        'Something went wrong. Please try again.';
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
              <h1 className="text-xl font-semibold tracking-tight text-ink-900">Forgot password</h1>
              <p className="mt-1 text-sm leading-relaxed text-ink-500">
                Enter your registered email and we&apos;ll send you a reset link.
              </p>

              {success ? (
                <div className="mt-6 rounded-xl border border-emerald-200 bg-emerald-50 px-4 py-4">
                  <p className="text-sm font-medium text-emerald-900">Check your email</p>
                  <p className="mt-1 text-sm leading-relaxed text-emerald-800">
                    If an account exists for that email, you&apos;ll receive a password reset link shortly.
                  </p>
                  <Link to="/login" className="mt-4 inline-flex text-sm font-medium text-dg-700 hover:text-dg-900 hover:underline focus-ring rounded">
                    Return to login
                  </Link>
                </div>
              ) : (
                <form onSubmit={handleSubmit(onSubmit)} noValidate className="mt-6 space-y-4">
                  {serverError && (
                    <div className="rounded-xl border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-800" role="alert">
                      {serverError}
                    </div>
                  )}
                  <Input label="Email" type="email" placeholder="you@example.com" autoComplete="email" required error={errors.email?.message} {...register('email')} />
                  <Button type="submit" loading={isSubmitting} className="w-full" size="lg">
                    Send reset link
                  </Button>
                  <p className="text-center text-sm text-ink-500">
                    Remembered it?{' '}
                    <Link to="/login" className="font-medium text-dg-700 hover:text-dg-900 hover:underline focus-ring rounded">
                      Log in
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
