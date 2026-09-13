import type { Metadata } from "next";

import { PortalChooser } from "@/components/student/PortalChooser";

export const metadata: Metadata = {
  title: "Your portal — Digi Guru",
  description: "Choose the Study Module or the Exam Module.",
  // Behind auth; keep it out of search indexes entirely.
  robots: { index: false, follow: false },
};

export default function PortalPage() {
  return (
    <div className="min-h-dvh bg-art-base text-lavender-50">
      <PortalChooser />
    </div>
  );
}
