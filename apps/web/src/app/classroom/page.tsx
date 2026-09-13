import type { Metadata } from "next";

import { ClassroomEntry } from "@/components/classroom/ClassroomEntry";

// Replaces the Phase 3 placeholder: the WebSocket session, the SyncGate client
// and the canvas now exist (`src/classroom/`). The real Gemini Live upstream is
// still stubbed server-side, so the tutor's turns come from the gateway's
// scripted demo — the NN-1 ordering this page proves is real either way, because
// the gate does not know or care where a turn came from.
//
// Always dark, independent of any dashboard preference: DESIGN.md §7 makes the
// classroom the one surface that does not follow the theme.

export const metadata: Metadata = {
  title: "Classroom — Digi Guru",
  description: "Your live tutoring session.",
  robots: { index: false, follow: false },
};

export default function ClassroomPage() {
  return <ClassroomEntry />;
}
