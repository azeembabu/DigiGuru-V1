import * as React from 'react';
import { useNavigate } from 'react-router-dom';
import { ProfileHeader } from '../components/profile/ProfileHeader';
import { StudentIdentityCard } from '../components/profile/StudentIdentityCard';
import { PersonalInformationCard } from '../components/profile/PersonalInformationCard';
import { AcademicInformationCard } from '../components/profile/AcademicInformationCard';
import { AccountInformationCard } from '../components/profile/AccountInformationCard';
import { ChangePasswordCard } from '../components/profile/ChangePasswordCard';
import { ActiveSessionsCard } from '../components/profile/ActiveSessionsCard';
import { LogoutCard } from '../components/profile/LogoutCard';
import { PreferencesCard } from '../components/profile/PreferencesCard';
import { ProfileSkeleton } from '../components/profile/ProfileSkeleton';
import { ProfileErrorState } from '../components/profile/ProfileErrorState';
import { useStudentProfile } from '../hooks/useStudentProfile';
import { useActiveSessions } from '../hooks/useActiveSessions';
import { clearTokens } from '../lib/api';
import { useAuthStore } from '../store/authStore';

/**
 * Student Profile.
 *
 * Identity is taken from the authenticated session only — the page never sends
 * a student id, so it can only ever read or change the signed-in student's own
 * record. No Admin navigation or controls exist here.
 *
 * Layout: on desktop the main column holds Personal Information, Academic
 * Information and Security, while the narrower secondary column holds the
 * Student Identity, Account Information and Preferences cards.
 *
 * Below the `lg` breakpoint the two column wrappers become `display: contents`,
 * so their cards join the grid directly and a single `order-*` sequence gives
 * the specified reading order: Student Identity → Personal Information →
 * Academic Information → Account Information → Security → Preferences.
 */
export default function StudentProfile() {
  const navigate = useNavigate();
  const logout = useAuthStore((s) => s.logout);

  const { profile, state, errorMessage, reload, save } = useStudentProfile();
  const { sessions, state: sessionsState, reload: reloadSessions, revoke } = useActiveSessions();

  const currentSession = React.useMemo(
    () => sessions.find((s) => s.isCurrent) ?? null,
    [sessions],
  );

  // Revoking the session you are using effectively signs you out.
  const handleSignedOut = React.useCallback(() => {
    clearTokens();
    logout();
    navigate('/login', { replace: true });
  }, [logout, navigate]);

  if (state === 'loading') return <ProfileSkeleton />;

  if (state === 'error' || !profile) {
    return <ProfileErrorState message={errorMessage ?? undefined} onRetry={() => void reload()} />;
  }

  return (
    <div className="mx-auto w-full max-w-6xl px-4 py-6 sm:px-6 lg:px-8">
      <ProfileHeader />

      <div className="grid gap-6 lg:grid-cols-3">
        {/* On mobile the column wrappers dissolve (`contents`), so each card below
            joins the grid directly and `order-*` sets the specified reading order.
            From `lg` up the wrappers become real flex columns and source order wins. */}
        <div className="contents lg:block lg:col-span-2 lg:space-y-6">
          <div className="order-2 lg:order-none">
            <PersonalInformationCard profile={profile} onSave={save} />
          </div>
          <div className="order-3 lg:order-none">
            <AcademicInformationCard profile={profile} />
          </div>

          <section className="order-5 space-y-6 lg:order-none" aria-labelledby="security-heading">
            <h2 id="security-heading" className="sr-only">
              Security
            </h2>
            <ChangePasswordCard />
            <ActiveSessionsCard
              sessions={sessions}
              state={sessionsState}
              onReload={reloadSessions}
              onRevoke={revoke}
              onSignedOut={handleSignedOut}
            />
            <LogoutCard />
          </section>
        </div>

        <div className="contents lg:block lg:space-y-6">
          <div className="order-1 lg:order-none">
            <StudentIdentityCard profile={profile} />
          </div>
          <div className="order-4 lg:order-none">
            <AccountInformationCard profile={profile} currentSession={currentSession} />
          </div>
          <div className="order-6 lg:order-none">
            <PreferencesCard currentSession={currentSession} />
          </div>
        </div>
      </div>
    </div>
  );
}
