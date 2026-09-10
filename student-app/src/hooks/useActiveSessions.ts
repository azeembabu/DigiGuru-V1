import * as React from 'react';
import { api } from '../lib/api';
import type { ActiveSession } from '../types/profile';

export type SessionsState = 'loading' | 'ready' | 'error';

interface RevokeResult {
  /** True when the session that was just revoked is the one making the call. */
  revokedCurrent: boolean;
}

interface UseActiveSessionsResult {
  sessions: ActiveSession[];
  state: SessionsState;
  reload: () => Promise<void>;
  /** Revoke one of the caller's own sessions. */
  revoke: (id: string) => Promise<RevokeResult>;
}

/**
 * Lists the authenticated student's own active sessions and revokes them.
 *
 * Every request is scoped server-side to the caller's user id, so a session id
 * belonging to another account is simply not found. The response never contains
 * refresh tokens or hashes.
 */
export function useActiveSessions(): UseActiveSessionsResult {
  const [sessions, setSessions] = React.useState<ActiveSession[]>([]);
  const [state, setState] = React.useState<SessionsState>('loading');

  const reload = React.useCallback(async () => {
    setState('loading');
    try {
      const { data: envelope } = await api.get('/auth/sessions');
      const list = (envelope?.data ?? envelope ?? []) as ActiveSession[];
      setSessions(list);
      setState('ready');
    } catch {
      setState('error');
    }
  }, []);

  React.useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const { data: envelope } = await api.get('/auth/sessions');
        if (cancelled) return;
        const list = (envelope?.data ?? envelope ?? []) as ActiveSession[];
        setSessions(list);
        setState('ready');
      } catch {
        if (cancelled) return;
        setState('error');
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const revoke = React.useCallback(
    async (id: string): Promise<RevokeResult> => {
      const { data: envelope } = await api.delete(`/auth/sessions/${id}`);
      const result = (envelope?.data ?? envelope ?? {}) as { revokedCurrent?: boolean };

      // Drop the row locally; no need to re-fetch the whole list for one revoke.
      setSessions((current) => current.filter((session) => session.id !== id));

      return { revokedCurrent: Boolean(result.revokedCurrent) };
    },
    [],
  );

  return { sessions, state, reload, revoke };
}
