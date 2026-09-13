import type { Metadata } from "next";
import { Suspense } from "react";

import { ClassroomEntry } from "@/components/classroom/ClassroomEntry";

// The live surface. Reached from `/classroom` with the block (and the unit
// within it) chosen; `?block=` is authoritative, and `students.current_block_id`
// is the fallback for a direct visit, so an existing "resume" link still works.

export const metadata: Metadata = {
  title: "Classroom — Digi Guru",
  description: "Your live tutoring session.",
  robots: { index: false, follow: false },
};

export default function ClassroomSessionPage() {
  return (
    <Suspense fallback={null}>
      <ClassroomEntry />
    </Suspense>
  );
}
