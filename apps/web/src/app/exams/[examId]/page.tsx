import type { Metadata } from "next";

import { ExamRunner } from "@/components/student/ExamRunner";

export const metadata: Metadata = {
  title: "Exam — Digi Guru",
  robots: { index: false, follow: false },
};

export default async function ExamRunnerPage({ params }: { params: Promise<{ examId: string }> }) {
  const { examId } = await params;
  return <ExamRunner examId={examId} />;
}
