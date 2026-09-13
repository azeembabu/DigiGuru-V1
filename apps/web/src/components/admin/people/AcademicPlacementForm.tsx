"use client";

import { useEffect, useState } from "react";

import { AdminButton, AdminField, AdminSelect } from "@/components/admin/controls";
import { Banner, panelClass } from "@/components/admin/primitives";
import { describeFailure } from "@/components/admin/people/placement-errors";
import { listLscs, listSemesters, updateStudentAcademic } from "@/lib/admin/client";
import type { Lsc, Program, Semester, Student } from "@/lib/admin/types";

/**
 * Program + semester + LSC, changed as one unit.
 *
 * The gateway revalidates the whole triple on every PATCH — `semester_id` must
 * belong to `program_id` or it answers `422 INVALID_SEMESTER` — so the three
 * are never split into separate saves. Changing the program clears the
 * semester rather than carrying a now-foreign id into the request: submitting
 * a mismatched pair would be a guaranteed 422, and the cleared select says why
 * before the admin presses anything.
 */
export function AcademicPlacementForm({
  student,
  programs,
  onSaved,
}: {
  student: Student;
  programs: Program[];
  onSaved: () => void;
}) {
  const [programId, setProgramId] = useState(student.program_id);
  const [semesterId, setSemesterId] = useState(student.semester_id);
  const [lscId, setLscId] = useState(student.lsc_id);

  const [semesters, setSemesters] = useState<Semester[]>([]);
  const [semestersLoading, setSemestersLoading] = useState(false);
  const [lscs, setLscs] = useState<Lsc[]>([]);

  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [fieldErrors, setFieldErrors] = useState<Record<string, string>>({});
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const page = await listLscs({ limit: 200 });
        if (!cancelled) setLscs(page.items);
      } catch {
        // A failed catalogue load degrades the centre picker to "cannot
        // change it" — it is not worth failing the whole form over.
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      if (!programId) {
        setSemesters([]);
        return;
      }
      setSemestersLoading(true);
      try {
        const rows = await listSemesters(programId);
        if (cancelled) return;
        setSemesters(rows);
        // Drop a selection the new program does not contain, instead of
        // sending the gateway a pair it will reject.
        setSemesterId((current) => (rows.some((row) => row.id === current) ? current : ""));
      } catch {
        if (!cancelled) setSemesters([]);
      } finally {
        if (!cancelled) setSemestersLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [programId]);

  const dirty =
    programId !== student.program_id ||
    semesterId !== student.semester_id ||
    lscId !== student.lsc_id;

  async function onSubmit(event: React.FormEvent) {
    event.preventDefault();
    setSaved(false);

    // Mirrors the gateway's own required-field check; it is not the
    // enforcement, only a round trip saved.
    const missing: Record<string, string> = {};
    if (!programId) missing.program_id = "Select a program.";
    if (!semesterId) missing.semester_id = "Select a semester for this program.";
    if (!lscId) missing.lsc_id = "Select a learner support centre.";
    if (Object.keys(missing).length > 0) {
      setFieldErrors(missing);
      setError(null);
      return;
    }

    setSaving(true);
    setError(null);
    setFieldErrors({});
    try {
      await updateStudentAcademic(student.id, {
        program_id: programId,
        semester_id: semesterId,
        lsc_id: lscId,
      });
      setSaved(true);
      onSaved();
    } catch (caught: unknown) {
      const failure = describeFailure(caught);
      setError(failure.message);
      setFieldErrors(failure.fieldErrors);
    } finally {
      setSaving(false);
    }
  }

  return (
    <section aria-labelledby="placement-heading" className={`${panelClass} p-5`}>
      <h2 id="placement-heading" className="font-display text-base font-semibold text-gray-900">
        Change academic placement
      </h2>
      <p className="mt-1 text-sm text-gray-500">
        Program, semester, and centre change together — the three are validated as one.
      </p>

      <form onSubmit={onSubmit} className="mt-4 space-y-4" noValidate>
        {error ? (
          <Banner tone="danger" onDismiss={() => setError(null)}>
            {error}
          </Banner>
        ) : null}
        {saved ? (
          <Banner tone="success" onDismiss={() => setSaved(false)}>
            Placement updated.
          </Banner>
        ) : null}

        <AdminField id="program_id" label="Program" required error={fieldErrors.program_id}>
          <AdminSelect
            id="program_id"
            value={programId}
            hasError={Boolean(fieldErrors.program_id)}
            onChange={(event) => setProgramId(event.target.value)}
          >
            <option value="">Select a program…</option>
            {programs.map((program) => (
              <option key={program.id} value={program.id}>
                {program.code} — {program.name}
              </option>
            ))}
          </AdminSelect>
        </AdminField>

        <AdminField
          id="semester_id"
          label="Semester"
          required
          error={fieldErrors.semester_id}
          hint={semestersLoading ? "Loading semesters…" : "Only semesters in the selected program."}
        >
          <AdminSelect
            id="semester_id"
            value={semesterId}
            disabled={!programId || semestersLoading}
            hasError={Boolean(fieldErrors.semester_id)}
            hasHint
            onChange={(event) => setSemesterId(event.target.value)}
          >
            <option value="">Select a semester…</option>
            {semesters.map((semester) => (
              <option key={semester.id} value={semester.id}>
                {semester.semester_number} — {semester.name}
              </option>
            ))}
          </AdminSelect>
        </AdminField>

        <AdminField id="lsc_id" label="Learner support centre" required error={fieldErrors.lsc_id}>
          <AdminSelect
            id="lsc_id"
            value={lscId}
            hasError={Boolean(fieldErrors.lsc_id)}
            onChange={(event) => setLscId(event.target.value)}
          >
            <option value="">Select a centre…</option>
            {lscs.map((lsc) => (
              <option key={lsc.id} value={lsc.id}>
                {lsc.code} — {lsc.name}
              </option>
            ))}
          </AdminSelect>
        </AdminField>

        <AdminButton type="submit" pending={saving} pendingLabel="Saving…" disabled={!dirty}>
          Save placement
        </AdminButton>
      </form>
    </section>
  );
}
