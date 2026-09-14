import type { Metadata } from "next";

import { AssessmentScreen } from "@/components/student/AssessmentScreen";

export const metadata: Metadata = {
  title: "My assessment — Digi Guru",
  description: "How you have performed on each block, with your own notes.",
  robots: { index: false, follow: false },
};

export default function AssessmentPage() {
  return <AssessmentScreen />;
}
