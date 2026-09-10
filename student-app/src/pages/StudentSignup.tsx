import * as React from 'react';
import { useForm } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';
import { Link, useNavigate } from 'react-router-dom';
import { signupSchema, type SignupInput } from '../lib/validations';
import { api, persistTokens } from '../lib/api';
import { useAuthStore } from '../store/authStore';
import { Button } from '../components/ui/Button';
import { Input } from '../components/ui/Input';
import { Select } from '../components/ui/Select';

interface RefOption {
  value: string;
  label: string;
  programId?: string;
}

interface RefRow {
  id: string;
  name: string;
  code?: string;
  semester_number?: number;
  program_id?: string;
}

export default function StudentSignup() {
  const navigate = useNavigate();
  const setUser = useAuthStore((s) => s.setUser);
  const [serverError, setServerError] = React.useState<string | null>(null);
  const [showPassword, setShowPassword] = React.useState(false);

  // Reference data from the backend — signup must submit real IDs (§11–§12).
  const [programOptions, setProgramOptions] = React.useState<RefOption[]>([]);
  const [semesterOptions, setSemesterOptions] = React.useState<RefOption[]>([]);
  const [lscOptions, setLscOptions] = React.useState<RefOption[]>([]);
  const [refsError, setRefsError] = React.useState<string | null>(null);

  React.useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const [p, s, l] = await Promise.all([
          api.get('/programs'),
          api.get('/semesters'),
          api.get('/lscs'),
        ]);
        if (cancelled) return;
        setProgramOptions(
          (p.data.data as RefRow[]).map((row) => ({ value: row.id, label: row.name })),
        );
        setSemesterOptions(
          (s.data.data as RefRow[]).map((row) => ({
            value: row.id,
            label: row.name ?? `Semester ${row.semester_number ?? ''}`,
            programId: row.program_id,
          })),
        );
        setLscOptions((l.data.data as RefRow[]).map((row) => ({ value: row.id, label: row.name })));
      } catch {
        if (!cancelled) setRefsError('Could not load programs/semesters/LSCs. Please refresh the page.');
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const {
    register,
    handleSubmit,
    resetField,
    watch,
    formState: { errors, isSubmitting },
  } = useForm<SignupInput>({
    resolver: zodResolver(signupSchema),
    defaultValues: {
      fullName: '',
      rollNumber: '',
      phoneNumber: '',
      email: '',
      program: '',
      semester: '',
      lsc: '',
      password: '',
      confirmPassword: '',
      terms: undefined,
    },
  });

  // A program owns its own semesters (e.g. every program has a "Semester 1"),
  // so the unfiltered list would show each semester name once per program.
  const selectedProgram = watch('program');
  const programSemesterOptions = React.useMemo(
    () => semesterOptions.filter((opt) => opt.programId === selectedProgram),
    [semesterOptions, selectedProgram],
  );

  // Changing program invalidates the previous semester selection.
  React.useEffect(() => {
    if (selectedProgram) resetField('semester');
  }, [selectedProgram, resetField]);

  async function onSubmit(values: SignupInput) {
    setServerError(null);
    try {
      const { data } = await api.post('/auth/student/signup', {
        full_name: values.fullName.trim(),
        roll_number: values.rollNumber.trim(),
        phone_number: values.phoneNumber.trim(),
        email: values.email.trim().toLowerCase(),
        program_id: values.program,
        semester_id: values.semester,
        lsc_id: values.lsc,
        password: values.password,
        // role is intentionally NOT sent — backend defaults to STUDENT
      });

      // Backend response envelope: { success, data: { accessToken, refreshToken, user } }
      const payload = data?.data ?? data;
      const accessToken: string | undefined = payload.accessToken ?? payload.access_token ?? payload.token;
      const refreshToken: string | undefined = payload.refreshToken ?? payload.refresh_token;
      const user = payload.user ?? payload.student ?? null;

      // If backend returns tokens, auto-login. Otherwise redirect to login.
      if (accessToken) {
        persistTokens({ accessToken, refreshToken: refreshToken ?? '' }, false);
        if (user) {
          setUser({
            id: String(user.id ?? user._id ?? values.rollNumber),
            fullName: user.fullName ?? values.fullName,
            rollNumber: user.rollNumber ?? values.rollNumber,
            email: user.email ?? values.email,
            phoneNumber: user.phoneNumber ?? values.phoneNumber,
            program: user.program ?? values.program,
            semester: user.semester ?? values.semester,
            lsc: user.lsc ?? values.lsc,
            role: 'STUDENT',
          });
        } else {
          setUser({
            id: values.rollNumber,
            fullName: values.fullName,
            rollNumber: values.rollNumber,
            email: values.email,
            phoneNumber: values.phoneNumber,
            program: values.program,
            semester: values.semester,
            lsc: values.lsc,
            role: 'STUDENT',
          });
        }
        navigate('/student/dashboard', { replace: true });
      } else {
        navigate('/login', { replace: true, state: { justSignedUp: true } });
      }
    } catch (err: unknown) {
      const resp = (err as { response?: { data?: { message?: string; error?: string; errors?: unknown } } })?.response?.data;
      const msg = resp?.message ?? resp?.error ?? (err as Error)?.message ?? 'Signup failed. Please try again.';
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
            to="/login"
            className="inline-flex items-center justify-center rounded-full border border-slate-200 bg-white/90 px-5 py-2.5 text-sm font-medium text-ink-700 transition hover:bg-white focus-ring"
          >
            Log in
          </Link>
        </header>

        {/* Signup form container */}
        <div className="flex flex-1 flex-col justify-center px-6 py-8 sm:px-10">
          <div className="mx-auto w-full max-w-[540px]">
            {/* Heading */}
            <div className="mb-8">
              <p className="font-display text-[11px] font-medium uppercase tracking-[0.18em] text-dg-700/80">Create your account</p>
              <h1 className="mt-2 font-display text-[28px] font-normal leading-tight text-ink-900 sm:text-[34px]">
                Your learning, <span className="font-medium text-dg-900">your textbook</span>
              </h1>
              <p className="mt-2 text-sm leading-relaxed text-ink-500 sm:text-[15px]">
                Admission takes 30 seconds. Your Roll Number is your identity — everything stays private to you.
              </p>
            </div>

            {/* Form card */}
            <div className="rounded-2xl border border-slate-200/50 bg-white/90 backdrop-blur-sm p-6 shadow-lg sm:p-8">
              <div className="mb-6">
                <h2 className="text-xl font-semibold tracking-tight text-ink-900">Create student account</h2>
                <p className="mt-1 text-sm text-ink-500">All fields are required. No role selection — you join as a Student.</p>
              </div>

              {refsError && (
                <div className="mb-5 rounded-xl border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-900" role="alert">
                  {refsError}
                </div>
              )}

              {serverError && (
                <div className="mb-5 rounded-xl border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-800" role="alert">
                  {serverError}
                </div>
              )}

              <form onSubmit={handleSubmit(onSubmit)} noValidate className="space-y-4">
                <div className="grid gap-4 sm:grid-cols-2">
                  <Input label="Full Name" placeholder="Anu Krishnan" autoComplete="name" required error={errors.fullName?.message} {...register('fullName')} />
                  <Input
                    label="Roll Number"
                    placeholder="012345"
                    autoComplete="off"
                    required
                    hint="6-digit number, e.g. 012345"
                    inputMode="numeric"
                    error={errors.rollNumber?.message}
                    {...register('rollNumber')}
                  />
                </div>

                <div className="grid gap-4 sm:grid-cols-2">
                  <Input
                    label="Phone Number"
                    placeholder="9876543210"
                    inputMode="numeric"
                    autoComplete="tel"
                    required
                    error={errors.phoneNumber?.message}
                    {...register('phoneNumber')}
                  />
                  <Input label="Email" placeholder="anu@example.com" type="email" autoComplete="email" required error={errors.email?.message} {...register('email')} />
                </div>

                <div className="grid gap-4 sm:grid-cols-3">
                  <Select
                    label="Program"
                    placeholder="Select program"
                    options={programOptions}
                    required
                    error={errors.program?.message}
                    {...register('program')}
                  />
                  <Select
                    label="Semester"
                    placeholder={selectedProgram ? 'Select semester' : 'Select program first'}
                    options={programSemesterOptions}
                    disabled={!selectedProgram}
                    required
                    error={errors.semester?.message}
                    {...register('semester')}
                  />
                  <Select
                    label="LSC"
                    placeholder="Select LSC"
                    options={lscOptions}
                    required
                    error={errors.lsc?.message}
                    {...register('lsc')}
                  />
                </div>

                <div className="grid gap-4 sm:grid-cols-2">
                  <div className="space-y-1.5">
                    <label htmlFor="password" className="block text-sm font-medium text-ink-900">
                      Password <span className="ml-1 text-red-500" aria-hidden>*</span>
                    </label>
                    <div className="relative">
                      <input
                        id="password"
                        type={showPassword ? 'text' : 'password'}
                        placeholder="At least 8 characters"
                        autoComplete="new-password"
                        aria-invalid={!!errors.password}
                        className={[
                          'block w-full rounded-xl border bg-white px-3.5 py-2.5 pr-16 text-[15px] text-ink-900 placeholder:text-ink-400',
                          'shadow-sm focus:outline-none focus:ring-2 focus:ring-dg-500/20 focus:border-dg-500',
                          errors.password ? 'border-red-300' : 'border-slate-200 hover:border-slate-300',
                        ].join(' ')}
                        {...register('password')}
                      />
                      <button
                        type="button"
                        onClick={() => setShowPassword((v) => !v)}
                        className="absolute inset-y-0 right-2 flex items-center rounded-lg px-2 text-xs font-medium text-ink-400 hover:text-ink-700 focus-ring"
                      >
                        {showPassword ? 'Hide' : 'Show'}
                      </button>
                    </div>
                    {errors.password?.message && (
                      <p className="text-sm text-red-600" role="alert">
                        {errors.password.message}
                      </p>
                    )}
                  </div>

                  <Input
                    label="Confirm Password"
                    placeholder="Repeat password"
                    type={showPassword ? 'text' : 'password'}
                    autoComplete="new-password"
                    required
                    error={errors.confirmPassword?.message}
                    {...register('confirmPassword')}
                  />
                </div>

                <label className="flex cursor-pointer items-start gap-2.5 rounded-xl border border-slate-200 bg-slate-50/70 px-3.5 py-3">
                  <input type="checkbox" className="mt-0.5 h-4 w-4 rounded border-slate-300 text-dg-600 focus:ring-dg-500" {...register('terms')} />
                  <span className="text-sm leading-relaxed text-ink-700">
                    I agree to the <span className="font-medium text-dg-700">Terms</span> and{' '}
                    <span className="font-medium text-dg-700">Privacy Policy</span>.
                  </span>
                </label>
                {errors.terms?.message && (
                  <p className="-mt-2 text-sm text-red-600" role="alert">
                    {errors.terms.message}
                  </p>
                )}

                <Button type="submit" loading={isSubmitting} className="w-full" size="lg">
                  Create Account
                </Button>

                <p className="text-center text-sm text-ink-500">
                  Already have an account?{' '}
                  <Link to="/login" className="font-medium text-dg-700 hover:text-dg-900 hover:underline focus-ring rounded">
                    Log in
                  </Link>
                </p>
              </form>
            </div>

            <p className="mt-6 max-w-lg text-center text-xs leading-relaxed text-ink-400">
              By creating an account, your data is separated by Roll Number. No other student can see your progress.
            </p>
          </div>
        </div>
      </div>
    </div>
  );
}
