import { Suspense } from "react";

import { StudentsList } from "@/components/admin/people/StudentsList";

// `StudentsList` reads `?status=` / `?q=` via `useSearchParams`, which opts its
// subtree out of static rendering — the boundary keeps that scoped to the list.
export default function AdminStudentsPage() {
  return (
    <Suspense>
      <StudentsList />
    </Suspense>
  );
}
