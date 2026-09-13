import { Suspense } from "react";

import { StudentReportsScreen } from "@/components/admin/reports/StudentReportsScreen";
import { TableSkeleton, panelClass } from "@/components/admin/primitives";

// Filters read from the query string (`useSearchParams`), which App Router
// requires be wrapped in a Suspense boundary — without one the whole route opts
// out of static rendering at build time.
export default function StudentReportsPage() {
  return (
    <Suspense
      fallback={
        <div className={`${panelClass} overflow-hidden`}>
          <TableSkeleton columns={9} />
        </div>
      }
    >
      <StudentReportsScreen />
    </Suspense>
  );
}
