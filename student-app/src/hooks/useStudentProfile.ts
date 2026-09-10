import * as React from 'react';
import { api } from '../lib/api';
import { useAuthStore } from '../store/authStore';
import type { StudentProfile, ProfileUpdate } from '../types/profile';

export type LoadState = 'loading' | 'ready' | 'error';

interface UseStudentProfileResult {
  profile: StudentProfile | null;
  state: LoadState;
  errorMessage: string | null;
  /** Re-fetch from scratch (drives the error-state Retry button). */
  reload: () => Promise<void>;
  /** Persist an update; resolves once the fresh profile has been applied. */
  save: (update: ProfileUpdate) => Promise<StudentProfile>;
}

/**
 * Loads and mutates the authenticated student's own profile.
 *
 * The student is identified solely by the access token — no id is ever sent,
 * so this can only ever read or change the caller's own record. On a successful
 * save the response is applied wholesale, so the UI never shows optimistic
 * values the server did not accept.
 */
export function useStudentProfile(): UseStudentProfileResult {
  const setUser = useAuthStore((s) => s.setUser);
  const currentUser = useAuthStore((s) => s.user);

  const [profile, setProfile] = React.useState<StudentProfile | null>(null);
  const [state, setState] = React.useState<LoadState>('loading');
  const [errorMessage, setErrorMessage] = React.useState<string | null>(null);

  const reload = React.useCallback(async () => {
    setState('loading');
    setErrorMessage(null);
    try {
      const { data: envelope } = await api.get('/student/profile');
      const next = (envelope?.data ?? envelope) as StudentProfile;
      setProfile(next);
      setState('ready');
    } catch {
      setState('error');
      setErrorMessage('We could not load your profile.');
    }
  }, []);

  React.useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const { data: envelope } = await api.get('/student/profile');
        if (cancelled) return;
        const next = (envelope?.data ?? envelope) as StudentProfile;
        setProfile(next);
        setState('ready');
      } catch {
        if (cancelled) return;
        setState('error');
        setErrorMessage('We could not load your profile.');
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const save = React.useCallback(
    async (update: ProfileUpdate): Promise<StudentProfile> => {
      const { data: envelope } = await api.patch('/student/profile', update);
      const next = (envelope?.data ?? envelope) as StudentProfile;
      setProfile(next);

      // Keep the header/avatar name in sync with the saved profile without
      // waiting for a page reload.
      if (currentUser && next.fullName !== currentUser.fullName) {
        setUser({ ...currentUser, fullName: next.fullName });
      }

      return next;
    },
    [currentUser, setUser],
  );

  return { profile, state, errorMessage, reload, save };
}
