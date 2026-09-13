import type { Metadata } from "next";

import { ExamListScreen } from "@/components/dashboard/ExamListScreen";

export const metadata: Metadata = {
  title: "Exam results — Digi Guru",
  description: "Every exam you have sat, newest first.",
  robots: { index: false, follow: false },
};

export default function ExamListPage() {
  return <ExamListScreen />;
}
