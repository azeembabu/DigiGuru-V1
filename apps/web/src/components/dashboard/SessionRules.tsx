import type { ComponentType } from "react";
import { Card, CardTitle } from "@/components/dashboard/Card";
import { IconBoard, IconBook, IconClock } from "@/components/icons";

// Student-facing phrasing only: these describe the guarantees the gateway
// enforces, but a student should never meet the internal names for them.
// Icon color follows DESIGN.md §4 on a dark surface — `lime-400` for the
// things that happen when you act, `indigo-400` for the purely informational.
const rules: {
  icon: ComponentType<{ className?: string }>;
  iconClass: string;
  title: string;
  body: string;
}[] = [
  {
    icon: IconBoard,
    iconClass: "text-lime-400",
    title: "You see it before you hear it",
    body: "Every explanation appears on the whiteboard before the tutor starts speaking, so you are never listening blind.",
  },
  {
    icon: IconClock,
    iconClass: "text-indigo-400",
    title: "20 minutes of speaking time",
    body: "The clock counts only while the voice session is live. You get a warning before the time is up, and the session then ends on its own.",
  },
  {
    icon: IconBook,
    iconClass: "text-indigo-400",
    title: "Only your textbooks",
    body: "The tutor answers from your own syllabus, and tells you plainly when a question falls outside it.",
  },
];

export function SessionRules() {
  return (
    <Card>
      <CardTitle>How a session works</CardTitle>

      <ul className="mt-5 flex flex-col gap-5">
        {rules.map(({ icon: Icon, iconClass, title, body }) => (
          <li key={title} className="flex gap-3">
            {/* The chip lifts the icon off the card fill, which on dark is too
                close in value for a bare glyph to register as a list marker. */}
            <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-[8px] border border-art-mid/60 bg-art-mid/50">
              <Icon className={`h-5 w-5 ${iconClass}`} />
            </span>
            <div>
              <h3 className="font-sans text-[15px] font-semibold leading-[1.5] text-white">
                {title}
              </h3>
              <p className="mt-1 text-[14px] leading-[1.5] text-gray-300/70">{body}</p>
            </div>
          </li>
        ))}
      </ul>
    </Card>
  );
}
