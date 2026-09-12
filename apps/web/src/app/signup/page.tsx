import type { Metadata } from "next";
import Link from "next/link";

import { AuthCard, AuthShell, type AuthFeature } from "@/components/auth/AuthShell";
import { SignupForm } from "@/components/auth/SignupForm";
import { IconBook, IconTarget, IconUsers } from "@/components/icons";
export const metadata: Metadata = {
  title: "Create your account — Digi Guru",
  description:
    "Register with your roll number, program, semester, and study centre to start live voice lessons taught from your own syllabus.",
};

const features: AuthFeature[] = [
  {
    icon: IconBook,
    title: "Personalised Learning Path",
    body: "Get content based on your program, semester and centre.",
  },
  {
    icon: IconUsers,
    title: "Expert Guidance",
    body: "Your tutor is bound to your syllabus, so answers are relevant and reliable.",
  },
  {
    icon: IconTarget,
    title: "Focused Progress",
    body: "Sessions are capped, keeping every lesson purposeful.",
  },
];

export default function SignupPage() {
  return (
    <AuthShell
      eyebrow={["Learn", "Interact", "Grow"]}
      tagline="Your Learning Partner"
      footnote={["Anytime", "Anywhere", "Always with you"]}
      title="Set it up once,"
      highlight="then just show up."
      blurb="Your program, semester, and study centre tell Digi Guru which textbooks to teach you from. After that, every session opens exactly where you left off."
      features={features}
    >
      <AuthCard
        heading="Create your account"
        sub="All fields are required. Your academic details must match your enrolment record."
        footer={
          <>
            Already registered?{" "}
            <Link
              href="/login"
              className="font-semibold text-lime-400 underline underline-offset-4 transition-colors hover:text-lime-300 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-300 focus-visible:ring-offset-2 focus-visible:ring-offset-art-base"
            >
              Sign in
            </Link>
          </>
        }
      >
        <SignupForm />
      </AuthCard>
    </AuthShell>
  );
}
