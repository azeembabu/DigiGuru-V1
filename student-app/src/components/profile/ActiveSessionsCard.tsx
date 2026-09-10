import * as React from 'react';
import { Laptop, Loader2, LogOut, RefreshCw, Smartphone } from 'lucide-react';
import { SectionCard } from './SectionCard';
import { Button } from '../ui/Button';
import { getApiErrorMessage } from '../../lib/api';
import { formatDateTime, formatRelativeTime } from '../../lib/format';
import type { ActiveSession } from '../../types/profile';

interface Props {
  sessions: ActiveSession[];
  state: 'loading' | 'ready' | 'error';
  onReload: () => Promise<void>;
  onRevoke: (id: string) => Promise<{ revokedCurrent: boolean }>;
  /** Called when the student revokes the session they are using right now. */
  onSignedOut: () => void;
}

function isMobileDevice(device: string): boolean {
  return /android|ios|iphone|ipad|mobile/i.test(device);
}

/**
 * Active sessions. Lists the student's own authenticated sessions and lets them
 * revoke any non-current one. No token, hash or raw user-agent is ever shown.
 */
export function ActiveSessionsCard({ sessions, state, onReload, onRevoke, onSignedOut }: Props) {
  const [revokingId, setRevokingId] = React.useState<string | null>(null);
  const [error, setError] = React.useState<string | null>(null);
  const [confirmId, setConfirmId] = React.useState<string | null>(null);

  async function handleRevoke(session: ActiveSession) {
    setError(null);
    setRevokingId(session.id);
    try {
      const { revokedCurrent } = await onRevoke(session.id);
      if (revokedCurrent) {
        onSignedOut();
        return;
      }
      setConfirmId(null);
    } catch (err) {
      setError(getApiErrorMessage(err, 'Could not sign out that session. Please try again.'));
    } finally {
      setRevokingId(null);
    }
  }

  return (
    <SectionCard
      title="Active Sessions"
      description="Devices currently signed in to your account."
      badge={
        state === 'ready' ? (
          <button
            type="button"
            onClick={() => void onReload()}
            className="inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] font-medium text-ink-500 transition-colors hover:bg-slate-100 hover:text-ink-900 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-dg-600"
          >
            <RefreshCw size={11} aria-hidden />
            Refresh
          </button>
        ) : undefined
      }
    >
      {error && (
        <div className="mb-4 rounded-lg border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-800" role="alert">
          {error}
        </div>
      )}

      {state === 'loading' && (
        <div className="flex items-center gap-3 py-6 text-sm text-ink-500" aria-busy="true" aria-live="polite">
          <Loader2 size={16} className="animate-spin" aria-hidden />
          Loading your sessions…
        </div>
      )}

      {state === 'error' && (
        <div className="rounded-lg border border-slate-200 bg-slate-50 px-4 py-4 text-sm text-ink-500">
          <p>We couldn&apos;t load your active sessions.</p>
          <Button variant="secondary" size="sm" className="mt-3" onClick={() => void onReload()}>
            Try again
          </Button>
        </div>
      )}

      {state === 'ready' && sessions.length === 0 && (
        <p className="py-2 text-sm text-ink-500">No active sessions found.</p>
      )}

      {state === 'ready' && sessions.length > 0 && (
        <ul className="divide-y divide-slate-100">
          {sessions.map((session) => {
            const Icon = isMobileDevice(session.device) ? Smartphone : Laptop;
            const isConfirming = confirmId === session.id;
            const isRevoking = revokingId === session.id;

            return (
              <li key={session.id} className="flex items-start justify-between gap-4 py-3.5 first:pt-0 last:pb-0">
                <div className="flex min-w-0 items-start gap-3">
                  <span className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-slate-100 text-ink-500">
                    <Icon size={15} aria-hidden />
                  </span>
                  <div className="min-w-0">
                    <p className="flex flex-wrap items-center gap-2 text-sm font-medium text-ink-900">
                      {session.device}
                      {session.isCurrent && (
                        <span className="rounded-full bg-dg-50 px-2 py-0.5 text-[11px] font-medium text-dg-800">
                          This device
                        </span>
                      )}
                    </p>
                    <p className="mt-0.5 text-xs text-ink-500">
                      Signed in {formatRelativeTime(session.createdAt)}
                      {session.ipAddress ? ` · ${session.ipAddress}` : ''}
                    </p>
                    <p className="mt-0.5 text-xs text-ink-400">
                      Expires {formatDateTime(session.expiresAt)}
                      {session.remembered ? ' · Remembered' : ''}
                    </p>
                  </div>
                </div>

                {isConfirming ? (
                  <div className="flex shrink-0 items-center gap-2">
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => setConfirmId(null)}
                      disabled={isRevoking}
                    >
                      Cancel
                    </Button>
                    <Button
                      variant="secondary"
                      size="sm"
                      onClick={() => void handleRevoke(session)}
                      loading={isRevoking}
                    >
                      Confirm
                    </Button>
                  </div>
                ) : (
                  <Button
                    variant="ghost"
                    size="sm"
                    className="shrink-0 text-red-600 hover:bg-red-50 hover:text-red-700"
                    onClick={() => {
                      setError(null);
                      setConfirmId(session.id);
                    }}
                    disabled={isRevoking}
                  >
                    <LogOut size={14} aria-hidden />
                    Revoke
                  </Button>
                )}
              </li>
            );
          })}
        </ul>
      )}
    </SectionCard>
  );
}
