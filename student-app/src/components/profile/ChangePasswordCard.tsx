import * as React from 'react';
import { useForm } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';
import { CheckCircle2, Eye, EyeOff, KeyRound } from 'lucide-react';
import { SectionCard } from './SectionCard';
import { Button } from '../ui/Button';
import { Input } from '../ui/Input';
import { changePasswordSchema, type ChangePasswordInput } from '../../lib/validations';
import { api, getApiErrorMessage, getStoredAccessToken } from '../../lib/api';

const REFRESH_KEY = 'dg_refresh_token';

/** Read the stored refresh token so the backend can keep this session alive. */
function readRefreshToken(): string | undefined {
  return (
    localStorage.getItem(REFRESH_KEY) ??
    sessionStorage.getItem(REFRESH_KEY) ??
    undefined
  );
}

/**
 * Change password. Requires the current password, which the backend re-verifies
 * independently. On success every *other* device is signed out.
 *
 * Passwords are held only in form state for the duration of the request and are
 * never persisted, logged, or echoed back.
 */
export function ChangePasswordCard() {
  const [serverError, setServerError] = React.useState<string | null>(null);
  const [successMessage, setSuccessMessage] = React.useState<string | null>(null);
  const [visible, setVisible] = React.useState(false);

  const {
    register,
    handleSubmit,
    reset,
    formState: { errors, isSubmitting },
  } = useForm<ChangePasswordInput>({
    resolver: zodResolver(changePasswordSchema),
    defaultValues: { currentPassword: '', newPassword: '', confirmPassword: '' },
  });

  // Never render forms for a session that isn't authenticated.
  const isAuthenticated = Boolean(getStoredAccessToken());

  async function onSubmit(values: ChangePasswordInput) {
    setServerError(null);
    setSuccessMessage(null);
    try {
      const { data: envelope } = await api.post('/auth/change-password', {
        currentPassword: values.currentPassword,
        newPassword: values.newPassword,
        confirmPassword: values.confirmPassword,
        // Lets the server keep *this* session while signing out other devices.
        refreshToken: readRefreshToken(),
      });

      const result = (envelope?.data ?? envelope ?? {}) as { otherSessionsRevoked?: number };
      const revoked = result.otherSessionsRevoked ?? 0;

      setSuccessMessage(
        revoked > 0
          ? `Password changed. ${revoked} other ${revoked === 1 ? 'session was' : 'sessions were'} signed out.`
          : 'Password changed successfully.',
      );
      reset();
    } catch (err) {
      setServerError(getApiErrorMessage(err, 'Could not change your password. Please try again.'));
    }
  }

  return (
    <SectionCard
      title="Change Password"
      description="Use a strong password you don't use anywhere else."
      badge={
        <span className="inline-flex items-center gap-1 rounded-full bg-slate-100 px-2 py-0.5 text-[11px] font-medium text-ink-500">
          <KeyRound size={11} aria-hidden />
          Required
        </span>
      }
    >
      {!isAuthenticated ? (
        <p className="text-sm text-ink-500">Please sign in again to change your password.</p>
      ) : (
        <form onSubmit={handleSubmit(onSubmit)} noValidate className="space-y-4">
          {serverError && (
            <div className="rounded-lg border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-800" role="alert">
              {serverError}
            </div>
          )}

          {successMessage && (
            <div
              className="flex items-center gap-2 rounded-lg border border-green-200 bg-green-50 px-4 py-3 text-sm text-green-800"
              role="status"
            >
              <CheckCircle2 size={16} aria-hidden />
              {successMessage}
            </div>
          )}

          <Input
            label="Current Password"
            type={visible ? 'text' : 'password'}
            autoComplete="current-password"
            error={errors.currentPassword?.message}
            {...register('currentPassword')}
          />

          <Input
            label="New Password"
            type={visible ? 'text' : 'password'}
            autoComplete="new-password"
            hint="At least 8 characters, with a letter and a number."
            error={errors.newPassword?.message}
            {...register('newPassword')}
          />

          <Input
            label="Confirm New Password"
            type={visible ? 'text' : 'password'}
            autoComplete="new-password"
            error={errors.confirmPassword?.message}
            {...register('confirmPassword')}
          />

          <label className="flex cursor-pointer items-center gap-2 text-sm text-ink-700">
            <input
              type="checkbox"
              checked={visible}
              onChange={(e) => setVisible(e.target.checked)}
              className="h-4 w-4 rounded border-slate-300 text-dg-600 focus:ring-dg-500"
            />
            <span className="inline-flex items-center gap-1.5">
              {visible ? <EyeOff size={14} aria-hidden /> : <Eye size={14} aria-hidden />}
              Show passwords
            </span>
          </label>

          <div className="border-t border-slate-100 pt-5">
            <Button type="submit" loading={isSubmitting}>
              Update Password
            </Button>
          </div>
        </form>
      )}
    </SectionCard>
  );
}
