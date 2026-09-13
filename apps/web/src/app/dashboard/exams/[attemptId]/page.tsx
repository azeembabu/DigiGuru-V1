import type { Metadata } from "next";

import { ExamReviewScreen } from "@/components/dashboard/ExamReviewScreen";

export const metadata: Metadata = {
  title: "Exam result — Digi Guru",
  // Behind auth, and the route carries an attempt id. Keep it out of indexes.
  robots: { index: false, follow: false },
};

// Next 16 hands route params to a page as a promise. The attempt id is the only
// thing in the URL — a foreign id resolves to nothing, so it identifies no one.
export default async function ExamReviewPage({
  params,
}: {
  params: Promise<{ attemptId: string }>;
}) {
  const { attemptId } = await params;
  return <ExamReviewScreen attemptId={attemptId} />;
}
