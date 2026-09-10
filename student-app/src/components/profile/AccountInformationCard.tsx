import { CalendarDays, Clock, MonitorSmartphone } from 'lucide-react';
import { SectionCard } from './SectionCard';
import { StatusBadge } from './StatusBadge';
import { formatDate, formatDateTime, formatRelativeTime } from '../../lib/format';
import type { StudentProfile, ActiveSession } from '../../types/profile';

interface Props {
  profile: StudentProfile;
  /** Current session, when the sessions list could be loaded. */
  currentSession: ActiveSession | null;
}

/**
 * Account metadata: when the account was created, the last login, current
 * status, and a short summary of the session being used right now.
 */
export function AccountInformationCard({ profile, currentSession }: Props) {
  return (
    <SectionCard title="Account Information" description="Details about your Digi Guru account.">
      <dl className="space-y-4">
        <div className="flex items-start justify-between gap-4">
          <dt className="flex items-center gap-2.5 text-sm text-ink-500">
            <CalendarDays size={16} className="shrink-0 text-ink-400" aria-hidden />
            Account created
          </dt>
          <dd className="text-right text-sm font-medium text-ink-900">
            {formatDate(profile.accountCreatedAt)}
          </dd>
        </div>

        <div className="flex items-start justify-between gap-4">
          <dt className="flex items-center gap-2.5 text-sm text-ink-500">
            <Clock size={16} className="shrink-0 text-ink-400" aria-hidden />
            Last login
          </dt>
          <dd className="text-right text-sm font-medium text-ink-900">
            {profile.lastLoginAt ? (
              <>
                {formatDateTime(profile.lastLoginAt)}
                <span className="mt-0.5 block text-xs font-normal text-ink-400">
                  {formatRelativeTime(profile.lastLoginAt)}
                </span>
              </>
            ) : (
              '—'
            )}
          </dd>
        </div>

        <div className="flex items-start justify-between gap-4">
          <dt className="flex items-center gap-2.5 text-sm text-ink-500">
            <MonitorSmartphone size={16} className="shrink-0 text-ink-400" aria-hidden />
            Current session
          </dt>
          <dd className="text-right text-sm font-medium text-ink-900">
            {currentSession ? (
              <>
                {currentSession.device}
                <span className="mt-0.5 block text-xs font-normal text-ink-400">
                  Signed in {formatRelativeTime(currentSession.createdAt)}
                </span>
              </>
            ) : (
              '—'
            )}
          </dd>
        </div>

        <div className="flex items-center justify-between gap-4">
          <dt className="text-sm text-ink-500">Account status</dt>
          <dd>
            <StatusBadge status={profile.accountStatus} />
          </dd>
        </div>
      </dl>
    </SectionCard>
  );
}
