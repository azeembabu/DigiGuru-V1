import type { Metadata } from "next";
import Link from "next/link";

import { Button } from "@/components/ui/Button";
import { IconBoard, IconClock } from "@/components/icons";
import { Logo } from "@/components/ui/Logo";

// The live classroom is Phase 3 (`IMPLEMENTATION_PLAN.md` §6) — the WebSocket
// session, SyncGate, and canvas do not exist yet. This placeholder holds the
// route so the dashboard CTA and any bookmarked link resolve instead of 404ing,
// and it is dark because DESIGN.md §7 makes the classroom the one surface that
// is *always* dark, independent of the student's dashboard preference. When the
// real session lands, this file is replaced, not extended.

export const metadata: Metadata = {
  title: "Classroom — Digi Guru",
  description: "Your live tutoring session.",
  robots: { index: false, follow: false },
};

export default function ClassroomPage() {
  return (
    <main className="flex min-h-dvh flex-col items-center justify-center bg-ink-950 px-5 py-16 text-center">
      <Logo size="md" />

      <h1 className="mt-8 max-w-xl text-balance font-display text-[28px] font-bold leading-[1.2] text-white sm:text-[40px] sm:leading-[1.1]">
        The classroom is <span className="text-lime-400">almost ready</span>
      </h1>
      <p className="mt-4 max-w-lg text-[18px] leading-[1.6] text-gray-300">
        Live voice sessions are still being built. Your program and current block are already saved,
        so the first session will open straight onto them — nothing for you to set up.
      </p>

      <ul className="mt-8 flex flex-col gap-3 text-left sm:flex-row sm:gap-8">
        <li className="flex items-center gap-2.5 text-[14px] leading-[1.5] text-gray-300">
          <IconBoard className="h-5 w-5 shrink-0 text-indigo-400" />
          Whiteboard before voice
        </li>
        <li className="flex items-center gap-2.5 text-[14px] leading-[1.5] text-gray-300">
          <IconClock className="h-5 w-5 shrink-0 text-indigo-400" />
          20-minute focused sessions
        </li>
      </ul>

      <div className="mt-10">
        <Button href="/dashboard" variant="outline">
          Back to dashboard
        </Button>
      </div>

      <p className="mt-6 text-[14px] leading-[1.5] text-gray-500">
        <Link href="/" className="underline underline-offset-4 hover:text-gray-300">
          Digi Guru home
        </Link>
      </p>
    </main>
  );
}
