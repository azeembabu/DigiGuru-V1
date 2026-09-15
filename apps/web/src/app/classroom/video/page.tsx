import type { Metadata } from "next";
import { Suspense } from "react";

import { UnitVideo } from "@/components/classroom/UnitVideo";

// The step between `/classroom` and `/classroom/session`, and only ever
// reached for a unit an admin has saved a video against — a unit without one is
// routed straight to the discussion, so this page is never an empty player.
//
// Always dark, like the rest of the classroom: DESIGN.md §7 makes this the one
// surface that does not follow the dashboard's theme.

export const metadata: Metadata = {
  title: "Classroom — Digi Guru",
  description: "Watch this unit's introduction before the discussion.",
  robots: { index: false, follow: false },
};

export default function ClassroomVideoPage() {
  // `useSearchParams` in the browser needs a Suspense boundary above it, or the
  // whole route opts out of static rendering.
  return (
    <Suspense fallback={null}>
      <UnitVideo />
    </Suspense>
  );
}
