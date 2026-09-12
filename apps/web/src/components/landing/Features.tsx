import type { ComponentType } from "react";
import { IconBoard, IconBook, IconClock, IconPin } from "@/components/icons";

const features: { icon: ComponentType<{ className?: string }>; title: string; body: string }[] = [
  {
    icon: IconBoard,
    title: "Board before voice",
    body: "Every explanation appears on the whiteboard first — you never listen blind, waiting to see what it means.",
  },
  {
    icon: IconBook,
    title: "Only your syllabus",
    body: "If it isn't in your textbook, the tutor says so — it never fills gaps with outside answers.",
  },
  {
    icon: IconClock,
    title: "Built for focus",
    body: "Sessions are capped at 20 minutes, so a lesson stays sharp instead of drifting into rambling.",
  },
  {
    icon: IconPin,
    title: "Cited, every time",
    body: "Each explanation opens with the exact chapter and page it's teaching from.",
  },
];

export function Features() {
  return (
    <section id="why" className="border-t border-ink-800 bg-ink-900/40">
      <div className="mx-auto max-w-6xl px-6 py-20 sm:py-28">
        <div className="max-w-xl">
          <p className="text-xs font-semibold tracking-[0.08em] text-indigo-300 uppercase">
            Why it&rsquo;s different
          </p>
          <h2 className="mt-4 text-balance font-display text-3xl font-bold text-lavender-50 sm:text-4xl">
            Built with rules <span className="text-lime-400">most AI tutors skip</span>
          </h2>
        </div>

        <div className="mt-14 grid gap-6 sm:grid-cols-2">
          {features.map(({ icon: Icon, title, body }) => (
            <div
              key={title}
              className="rounded-md border border-ink-800 bg-ink-900 p-6 shadow-[inset_0_1px_0_0_rgba(255,255,255,0.04)]"
            >
              <Icon className="h-6 w-6 text-lime-400" />
              <h3 className="mt-4 font-display text-lg font-semibold text-lavender-50">{title}</h3>
              <p className="mt-2 text-[15px] leading-relaxed text-gray-300">{body}</p>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}
