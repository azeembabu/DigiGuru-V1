import type { ReactNode } from "react";

import { DashboardHeader } from "@/components/dashboard/DashboardHeader";
import { DashboardSidebar } from "@/components/dashboard/DashboardSidebar";

// The exam module shares the dashboard's shell, deliberately: it is the same
// student in the same place, and a second chrome would make the two modules
// feel like two products. The rail is fixed, so the content column carries its
// own left offset rather than living in a flex row.
export default function ExamsLayout({ children }: { children: ReactNode }) {
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
