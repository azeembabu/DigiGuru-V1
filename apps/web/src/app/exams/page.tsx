import type { Metadata } from "next";

import { ExamsScreen } from "@/components/student/ExamsScreen";

export const metadata: Metadata = {
  title: "Exams — Digi Guru",
  description: "Assignments, mid-term quizzes and semester exams for your semester.",
  robots: { index: false, follow: false },
};

export default function ExamsPage() {
  return <ExamsScreen />;
}
