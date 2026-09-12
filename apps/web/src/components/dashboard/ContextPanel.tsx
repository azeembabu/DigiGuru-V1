import type { ReactNode } from "react";
import { cardClass, CardTitle } from "@/components/dashboard/Card";
import type { StudentContext } from "@/lib/student-context";

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-1 border-b border-art-mid/60 py-3 last:border-b-0 sm:flex-row sm:items-baseline sm:justify-between sm:gap-6">
      <dt className="text-[14px] leading-[1.5] text-gray-300/70">{label}</dt>
      <dd className="text-[15px] font-semibold leading-[1.5] text-white sm:text-right">
        {children}
      </dd>
    </div>
  );
}

export function ContextPanel({ context }: { context: StudentContext }) {
  const { program, semester, current_block: block } = context;
  // The API can hand back a semester with no display name; the ordinal is the
  // one field that is always present, so it carries the fallback.
  const semesterLabel = semester.name.trim() || `Semester ${semester.semester_number}`;

  return (
    // Built from `cardClass` rather than <Card> because the sidebar's
    // `#enrolment` link has to land on the panel's own root element, and
    // `scroll-mt-24` keeps the sticky page header off the heading.
    <section id="enrolment" className={`${cardClass} scroll-mt-24`}>
      <CardTitle>Your enrolment</CardTitle>

      <dl className="mt-4">
        <Row label="Program">
          {program.name} <span className="font-normal text-gray-300/60">{program.code}</span>
        </Row>
        <Row label="Semester">{semesterLabel}</Row>
        <Row label="Current block">
          {block === null ? (
            <span className="font-normal text-gray-300/60">&mdash; Not assigned yet</span>
          ) : (
            <>
              {block.title} &mdash; Block {block.block_no}
            </>
          )}
        </Row>
      </dl>

      {/* Self-service PATCH on `/me/profile` allow-lists only `full_name` and
          `phone_number` (apps/gateway/src/me/profile.rs), so say so here rather
          than let a student hunt for an edit control that does not exist. */}
      <p className="mt-4 text-[14px] leading-[1.5] text-gray-300/70">
        Your academic details are set by your learning centre and cannot be changed from here.
      </p>
    </section>
  );
}
