import type { ReactNode } from "react";

import { DashboardHeader } from "@/components/dashboard/DashboardHeader";
import { DashboardSidebar } from "@/components/dashboard/DashboardSidebar";

// Shares the dashboard shell for the same reason the exam module does: it is
// the same student in the same place, and a second chrome would make this feel
// like a separate product.
export default function AssessmentLayout({ children }: { children: ReactNode }) {
  return (
    <div className="min-h-dvh bg-art-base text-lavender-50">
      <DashboardSidebar />
      <div className="lg:pl-60">
        <DashboardHeader />
        <main className="mx-auto w-full max-w-5xl px-5 py-8 sm:px-8">{children}</main>
      </div>
    </div>
  );
}
