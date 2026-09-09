import * as React from 'react';
import { useNavigate } from 'react-router-dom';
import { api, clearTokens } from '../lib/api';
import { useAuthStore } from '../store/authStore';
import { Card } from '../components/ui/Card';
import { Button } from '../components/ui/Button';

/**
 * Student settings (§20): authentication-related only — password (later),
 * session info, logout. AI learning preferences belong to a later phase.
 */
export default function StudentSettings() {
  const navigate = useNavigate();
  const user = useAuthStore((s) => s.user);
  const logout = useAuthStore((s) => s.logout);
  const [isLoggingOut, setIsLoggingOut] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);

  async function handleLogout() {
    setError(null);
    setIsLoggingOut(true);
    try {
      // Backend logout revokes the session (Phase 2 endpoint)
      await api.post('/auth/logout');
    } catch {
      // Session may already be expired — still clear local state
    } finally {
      clearTokens();
      logout();
      navigate('/login', { replace: true });
    }
  }

  return (
    <div className="mx-auto max-w-2xl px-4 py-8 sm:px-6 lg:px-8">
      <p className="text-xs font-medium uppercase tracking-[0.14em] text-dg-700/80">Settings</p>
      <h1 className="mt-1 font-display text-2xl font-medium text-ink-900">Account settings</h1>
      <p className="mt-1 text-sm text-ink-500">Authentication and session controls. Signed in as {user?.email ?? '—'}.</p>

      {error && (
        <div className="mt-6 rounded-xl border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-800" role="alert">
          {error}
        </div>
      )}

      <div className="mt-8 space-y-6">
        {/* Password */}
        <Card variant="solid" padding="md">
          <h2 className="text-base font-semibold text-ink-900">Password</h2>
          <p className="mt-1 text-sm text-ink-500">
            Change your password from the reset flow — use “Forgot password?” on the login page.
          </p>
          <p className="mt-3 inline-flex rounded-full bg-slate-100 px-2.5 py-1 text-[11px] font-medium text-ink-500">
            Full change-password flow coming soon
          </p>
        </Card>

        {/* Sessions */}
        <Card variant="solid" padding="md">
          <h2 className="text-base font-semibold text-ink-900">Sessions</h2>
          <p className="mt-1 text-sm text-ink-500">
            Your login is protected by a secure session. Logging out revokes it on the server, so the token can&apos;t be reused.
          </p>
          <p className="mt-3 inline-flex rounded-full bg-slate-100 px-2.5 py-1 text-[11px] font-medium text-ink-500">
            Device list coming soon
          </p>
        </Card>

        {/* Logout */}
        <Card variant="solid" padding="md">
          <h2 className="text-base font-semibold text-ink-900">Log out</h2>
          <p className="mt-1 text-sm text-ink-500">Ends this session on the server and returns you to the login page.</p>
          <Button variant="secondary" className="mt-4" onClick={handleLogout} loading={isLoggingOut}>
            Log out
          </Button>
        </Card>
      </div>
    </div>
  );
}
