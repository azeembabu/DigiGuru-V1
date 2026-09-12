import type { Metadata } from "next";

import { StudentDashboard } from "@/components/dashboard/StudentDashboard";

export const metadata: Metadata = {
  title: "Dashboard — Digi Guru",
  description: "Your program, your current block, and the way back into the classroom.",
  // The dashboard is behind auth; keep it out of search indexes entirely.
  robots: { index: false, follow: false },
};

export default function DashboardPage() {
  return <StudentDashboard />;
}
