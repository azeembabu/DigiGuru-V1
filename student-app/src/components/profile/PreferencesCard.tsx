import { Clock } from 'lucide-react';
import { SectionCard } from './SectionCard';
import type { ActiveSession } from '../../types/profile';

interface Props {
  /** The session in use right now, used to report the active session length. */
  currentSession: ActiveSession | null;
}

/**
 * Preferences.
 *
 * Digi Guru currently supports exactly one account preference a student can
 * influence — whether a session is remembered (a 30-day session) or short
 * (8 hours), chosen with "Remember me" at login. That choice is a per-session
 * setting rather than a stored profile field, so it is reported here, not
 * edited. No toggles are invented for settings the backend cannot persist.
 */
export function PreferencesCard({ currentSession }: Props) {
  const sessionLabel = currentSession
    ? currentSession.remembered
      ? 'Remembered (30 days)'
      : 'Standard (8 hours)'
    : '—';

  return (
    <SectionCard title="Preferences" description="Account preferences currently supported.">
      <div className="flex items-start justify-between gap-4">
        <div className="flex items-start gap-2.5">
          <Clock size={16} className="mt-0.5 shrink-0 text-ink-400" aria-hidden />
          <div>
            <p className="text-sm text-ink-700">Session length</p>
            <p className="mt-0.5 text-xs leading-relaxed text-ink-400">
              Chosen with “Remember me” when you sign in.
            </p>
          </div>
        </div>
        <p className="shrink-0 text-right text-sm font-medium text-ink-900">{sessionLabel}</p>
      </div>

      <div className="mt-4 rounded-lg border border-dashed border-slate-200 bg-slate-50/70 px-4 py-3">
        <p className="text-xs leading-relaxed text-ink-500">
          Additional preferences aren&apos;t available yet. When they are added, they&apos;ll appear here.
        </p>
      </div>
    </SectionCard>
  );
}
