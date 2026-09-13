import type { Metadata } from "next";

import { AnswerSheetView } from "@/components/student/AnswerSheet";

export const metadata: Metadata = {
  title: "Answer sheet — Digi Guru",
  description: "Your marked answers, the correct answers, and why.",
  // Never indexed: the page renders a named student's marks.
  robots: { index: false, follow: false },
};

export default async function AnswerSheetPage({
  params,
}: {
  params: Promise<{ attemptId: string }>;
}) {
  const { attemptId } = await params;
  return <AnswerSheetView attemptId={attemptId} />;
}
