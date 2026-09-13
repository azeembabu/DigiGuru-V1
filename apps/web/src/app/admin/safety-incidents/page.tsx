import { Suspense } from "react";

import { SafetyIncidentsScreen } from "@/components/admin/drilldown/SafetyIncidentsScreen";
import { TableSkeleton, panelClass } from "@/components/admin/primitives";

// The screen reads its filters from the query string with `useSearchParams`,
// which App Router requires be wrapped in a Suspense boundary — without one the
// whole route opts out of static rendering at build time.
export default function SafetyIncidentsPage() {
  return (
    <Suspense
      fallback={
        <div className={`${panelClass} overflow-hidden`}>
          <TableSkeleton columns={6} />
        </div>
      }
    >
      <SafetyIncidentsScreen />
    </Suspense>
  );
}
