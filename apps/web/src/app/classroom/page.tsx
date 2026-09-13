import type { Metadata } from "next";
import { Suspense } from "react";

import { SyllabusBrowser } from "@/components/classroom/SyllabusBrowser";

// `/classroom` is the way *into* a session, not the session itself: course ->
// block -> unit, then `/classroom/session`. It opened the board directly on
// `students.current_block_id` before, which let a student study one block and
// gave them no way to reach the rest of their own syllabus.
//
// Always dark, independent of any dashboard preference: DESIGN.md §7 makes the
// classroom the one surface that does not follow the theme.

export const metadata: Metadata = {
  title: "Classroom — Digi Guru",
  description: "Choose what to study.",
  robots: { index: false, follow: false },
};

export default function ClassroomPage() {
  // `useSearchParams` in the browser requires a Suspense boundary above it, or
  // the whole route opts out of static rendering.
  return (
    <Suspense fallback={null}>
      <SyllabusBrowser />
    </Suspense>
  );
}
