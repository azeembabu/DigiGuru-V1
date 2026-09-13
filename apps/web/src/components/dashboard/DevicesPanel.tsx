"use client";

import { useEffect, useState } from "react";

import { cardClass, CardLabel, CardTitle } from "@/components/dashboard/Card";
import { IconDevice } from "@/components/icons";
import { ApiError } from "@/lib/api";
import {
  loadAuthSessions,
  revokeAuthSession,
  type AuthSessionSummary,
} from "@/lib/student-context";

// Dates are formatted with an explicit locale *and* time zone rather than the
// viewer's. `Intl.DateTimeFormat` with no arguments resolves differently on the
// Node render than in the browser, and the mismatched text is exactly the kind
// of thing React flags as a hydration error. Asia/Kolkata is the one time zone
// the students and the admins share, so a fixed zone is also the honest reading
// of "signed in at" for this product — not a compromise for hydration's sake.
const stamp = new Intl.DateTimeFormat("en-IN", {
  dateStyle: "medium",
  timeStyle: "short",
  timeZone: "Asia/Kolkata",
});

function formatCreatedAt(iso: string): string {
  const at = new Date(iso);
  // The gateway sends RFC 3339, but a bad row should degrade to the raw value
  // rather than render "Invalid Date" next to a sign-out button.
  return Number.isNaN(at.getTime()) ? iso : stamp.format(at);
}

function messageOf(error: unknown): string {
  return error instanceof ApiError
    ? error.message
    : "Something went wrong. Please try again.";
}

// The focus-ring offset colour has to match what the ring actually sits on
// (DESIGN.md §8) — here that is the card fill, not the page ground.
const rowButton =
  "shrink-0 rounded-full border-[1.5px] border-art-mid/60 px-4 py-2 font-sans text-[14px] " +
  "font-semibold text-white transition-colors hover:border-art-edge/60 hover:bg-art-mid/50 " +
  "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-300 " +
  "focus-visible:ring-offset-2 focus-visible:ring-offset-art-deep " +
  "disabled:cursor-not-allowed disabled:opacity-60";

export function DevicesPanel() {
  const [sessions, setSessions] = useState<AuthSessionSummary[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);
  // Keyed by session id so two rows can be in flight, or failing, independently.
  const [revoking, setRevoking] = useState<Record<string, boolean>>({});
  const [rowErrors, setRowErrors] = useState<Record<string, string>>({});

  useEffect(() => {
    let cancelled = false;

    setSessions(null);
    setLoadError(null);

    loadAuthSessions()
      .then((result) => {
        if (!cancelled) setSessions(result);
      })
      .catch((error: unknown) => {
        if (!cancelled) setLoadError(messageOf(error));
      });

    // A "Try again" fired while the first request is still open would otherwise
    // let the slower response win and overwrite the newer one.
    return () => {
      cancelled = true;
    };
  }, [attempt]);

  async function onRevoke(id: string) {
    if (revoking[id]) return;
    setRevoking((prev) => ({ ...prev, [id]: true }));
    setRowErrors((prev) => {
      if (!(id in prev)) return prev;
      const next = { ...prev };
      delete next[id];
      return next;
    });

    try {
      await revokeAuthSession(id);
      setSessions((prev) => (prev ? prev.filter((s) => s.id !== id) : prev));
    } catch (error: unknown) {
      setRowErrors((prev) => ({ ...prev, [id]: messageOf(error) }));
      setRevoking((prev) => ({ ...prev, [id]: false }));
    }
  }

  return (
    // Built from `cardClass` rather than <Card> because the sidebar's
    // `#security` link has to land on the panel's own root element, and
    // `scroll-mt-24` keeps the sticky page header off the heading.
    <section id="security" className={`${cardClass} scroll-mt-24`}>
      <CardLabel>Account security</CardLabel>
      <CardTitle>Where you&rsquo;re signed in</CardTitle>

      {loadError !== null ? (
        <div className="mt-4">
          <p className="text-[14px] leading-[1.5] text-gray-300">{loadError}</p>
          <button
            type="button"
            onClick={() => setAttempt((n) => n + 1)}
            className={`${rowButton} mt-3`}
          >
            Try again
          </button>
        </div>
      ) : sessions === null ? (
        <p aria-busy="true" className="mt-4 text-[14px] leading-[1.5] text-gray-300/70">
          Loading your active sessions…
        </p>
      ) : sessions.length === 0 ? (
        <p className="mt-4 text-[14px] leading-[1.5] text-gray-300/70">
          No other active sessions.
        </p>
      ) : (
        <ul className="mt-4 divide-y divide-art-mid/60">
          {sessions.map((session) => (
            <li key={session.id} className="flex items-start gap-4 py-4 first:pt-0 last:pb-0">
              <span className="mt-0.5 flex h-9 w-9 shrink-0 items-center justify-center rounded-[8px] border border-art-mid/60 bg-art-mid/50">
                <IconDevice className="h-5 w-5 text-gray-300/70" />
              </span>

              <div className="min-w-0 flex-1">
                <p className="truncate text-[16px] font-semibold leading-[1.5] text-white">
                  {session.device_info ?? "Unknown device"}
                </p>
                <p className="text-[14px] leading-[1.5] text-gray-300/70">
                  Signed in {formatCreatedAt(session.created_at)}
                  {/* The student's own IP, kept muted: it is the detail that makes
                      an unfamiliar session recognisable, not something to lead with. */}
                  {session.ip_address ? ` · ${session.ip_address}` : ""}
                </p>
                {rowErrors[session.id] ? (
                  <p className="mt-1 text-[14px] leading-[1.5] text-danger">
                    {rowErrors[session.id]}
                  </p>
                ) : null}
              </div>

              <button
                type="button"
                onClick={() => onRevoke(session.id)}
                disabled={Boolean(revoking[session.id])}
                aria-busy={Boolean(revoking[session.id])}
                className={rowButton}
              >
                Sign out
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
