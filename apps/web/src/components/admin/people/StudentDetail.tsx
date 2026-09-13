"use client";

import Link from "next/link";
import { useCallback, useEffect, useState } from "react";

import { useAdminSession } from "@/components/admin/AdminShell";
import { AdminButton } from "@/components/admin/controls";
import {
  Banner,
  EmptyState,
  PageHeader,
  StatusPill,
  panelClass,
} from "@/components/admin/primitives";
import { AcademicPlacementForm } from "@/components/admin/people/AcademicPlacementForm";
import { CurrentBlockForm } from "@/components/admin/people/CurrentBlockForm";
import { formatDate, nameOf, useCatalogue } from "@/components/admin/people/lookups";
import { getStudent } from "@/lib/admin/client";
import type { Student } from "@/lib/admin/types";
import { ApiError } from "@/lib/api";
import { inScope } from "@/lib/me";

/**
 * One student: who they are (read-only) and where they sit in the syllabus
 * (editable).
 *
 * Identity is not editable from here on purpose — the gateway exposes no admin
 * route that writes a student's name, phone, or email; a student edits those
 * themselves through `PATCH /me/profile`.
 */
export function StudentDetail({ studentId }: { studentId: string }) {
  const { me } = useAdminSession();
  const catalogue = useCatalogue();

  const [student, setStudent] = useState<Student | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setStudent(await getStudent(studentId));
      setError(null);
    } catch (caught: unknown) {
      setStudent(null);
      setError(
        caught instanceof ApiError
          ? caught.message
          : "Could not load this student.",
      );
    } finally {
      setLoading(false);
    }
  }, [studentId]);

  useEffect(() => {
    // Wrapped rather than called directly: `load` sets state, and React 19's
    // lint rule counts a synchronous setState in an effect body as a cascade.
    void (async () => {
      await load();
    })();
  }, [load]);

  const backLink = (
    <Link
      href="/admin/students"
      className="rounded-sm text-sm font-medium text-indigo-500 underline-offset-2 hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-400 focus-visible:ring-offset-2 focus-visible:ring-offset-lavender-50"
    >
      ← All students
    </Link>
  );

  if (loading) {
    return (
      <div className="space-y-6">
        {backLink}
        <p role="status" className="text-sm text-gray-500">
          Loading student…
        </p>
      </div>
    );
  }

  if (error || !student) {
    return (
      <div className="space-y-4">
        {backLink}
        <Banner tone="danger">{error ?? "This student could not be found."}</Banner>
        <AdminButton type="button" onClick={() => void load()}>
          Try again
        </AdminButton>
      </div>
    );
  }

  // A sub-admin can reach this page for a student in scope only; the gateway
  // enforces that, but if a scope was revoked between list and detail the
  // forms below would 403 on submit — say so instead.
  const editable = inScope(me, student.program_id);

  return (
    <div className="space-y-6">
      {backLink}

      <PageHeader
        title={student.full_name}
        description={`Roll number ${student.roll_number}`}
      />

      {!editable ? (
        <Banner tone="info">
          This student is outside the programs you are scoped to, so their placement is
          read-only for you.
        </Banner>
      ) : null}

      <section aria-labelledby="identity-heading" className={`${panelClass} p-5`}>
        <h2 id="identity-heading" className="font-display text-base font-semibold text-gray-900">
          Identity
        </h2>
        <dl className="mt-4 grid gap-x-8 gap-y-4 sm:grid-cols-2 lg:grid-cols-3">
          <Detail label="Full name" value={student.full_name} />
          <Detail label="Roll number" value={student.roll_number} mono />
          <Detail label="Phone" value={student.phone_number} mono />
          <Detail label="Program" value={nameOf(catalogue.programNames, student.program_id)} />
          <Detail label="Learner support centre" value={nameOf(catalogue.lscNames, student.lsc_id)} />
          <div>
            <dt className="text-xs font-medium uppercase tracking-wide text-gray-500">
              First login
            </dt>
            <dd className="mt-1">
              {student.is_first_login ? (
                <StatusPill tone="info">Not yet signed in</StatusPill>
              ) : (
                <StatusPill tone="success">Completed</StatusPill>
              )}
            </dd>
          </div>
          <Detail label="Registered" value={formatDate(student.created_at)} />
          <Detail label="Last updated" value={formatDate(student.updated_at)} />
        </dl>
      </section>

      {editable ? (
        <>
          <AcademicPlacementForm
            student={student}
            programs={catalogue.programs}
            onSaved={() => void load()}
          />
          <CurrentBlockForm
            student={student}
            programs={catalogue.programs}
            onSaved={() => void load()}
          />
        </>
      ) : (
        <div className={panelClass}>
          <EmptyState
            title="Placement cannot be changed here"
            hint="Ask a super admin, or one scoped to this student's program."
          />
        </div>
      )}
    </div>
  );
}

function Detail({ label, value, mono = false }: { label: string; value: string; mono?: boolean }) {
  return (
    <div>
      <dt className="text-xs font-medium uppercase tracking-wide text-gray-500">{label}</dt>
      <dd className={`mt-1 text-sm text-gray-900 ${mono ? "font-mono" : ""}`}>{value}</dd>
    </div>
  );
}
