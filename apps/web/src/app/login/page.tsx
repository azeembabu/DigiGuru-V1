import type { Metadata } from "next";
import Link from "next/link";
import { Suspense } from "react";

import { AuthCard, AuthShell, type AuthFeature } from "@/components/auth/AuthShell";
import { LoginForm } from "@/components/auth/LoginForm";
import { IconBoard, IconBook, IconClock } from "@/components/icons";
export const metadata: Metadata = {
  title: "Sign in — Digi Guru",
  description:
    "Sign in to Digi Guru to pick up your lesson where you left off, taught live from your own syllabus.",
};

const features: AuthFeature[] = [
  {
    icon: IconClock,
    title: "Pick Up Where You Stopped",
    body: "A short recap brings you back to the point your last session ended.",
  },
  {
    icon: IconBoard,
    title: "You See It Before You Hear It",
    body: "Every explanation lands on the whiteboard before the tutor speaks.",
  },
  {
    icon: IconBook,
    title: "Only Your Syllabus",
    body: "Your tutor teaches from the textbooks set for your course, and says so when a question falls outside them.",
  },
];

export default function LoginPage() {
  return (
    <AuthShell
      eyebrow={["Learn", "Interact", "Grow"]}
      tagline="Your Learning Partner"
      footnote={["Anytime", "Anywhere", "Always with you"]}
      title="Your tutor remembers"
      highlight="where you stopped."
      blurb="Sign in and the classroom opens straight onto your program, semester, and current block — no picker, no setup."
      features={features}
    >
      <AuthCard
        heading="Sign in"
        sub="Use the email address you registered with."
        footer={
          <>
            New to Digi Guru?{" "}
            <Link
              href="/signup"
              className="font-semibold text-lime-400 underline underline-offset-4 transition-colors hover:text-lime-300 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-300 focus-visible:ring-offset-2 focus-visible:ring-offset-art-base"
            >
              Create an account
            </Link>
          </>
        }
      >
        {/* `LoginForm` reads `?next=` to send an admin back to the page that
            bounced them, and `useSearchParams` opts its subtree out of static
            rendering — the boundary keeps that scoped to the form instead of
            the whole page. */}
        <Suspense fallback={null}>
          <LoginForm />
        </Suspense>
      </AuthCard>
    </AuthShell>
  );
}
