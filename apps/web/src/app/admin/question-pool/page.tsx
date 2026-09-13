import { Suspense } from "react";

import { QuestionPoolScreen } from "@/components/admin/questions/QuestionPoolScreen";
import { TableSkeleton, panelClass } from "@/components/admin/primitives";

// Filters live in the query string (`useSearchParams`), which App Router
// requires be wrapped in a Suspense boundary — without one the whole route opts
// out of static rendering at build time.
export default function QuestionPoolPage() {
  return (
    <Suspense
      fallback={
        <div className={`${panelClass} overflow-hidden`}>
          <TableSkeleton columns={7} />
        </div>
      }
    >
      <QuestionPoolScreen />
    </Suspense>
  );
}
