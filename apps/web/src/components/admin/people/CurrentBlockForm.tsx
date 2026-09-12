"use client";

import { useEffect, useState } from "react";

import { AdminButton, AdminField, AdminSelect } from "@/components/admin/controls";
import { Banner, panelClass } from "@/components/admin/primitives";
import { describeFailure } from "@/components/admin/people/placement-errors";
import { listBlocks, listCourses, listSemesters, setStudentBlock } from "@/lib/admin/client";
import type { Block, Course, Program, Semester, Student } from "@/lib/admin/types";

/**
 * The current-block picker walks the academic hierarchy
 * (Program > Semester > Course > Block) because the gateway only ever lists
 * one level at a time — there is no "blocks for this student" endpoint, and a
 * `block_id` is the only thing the PATCH accepts.
 *
 * It starts from the student's own program/semester rather than an empty
 * picker: moving a student to a block outside their placement is possible but
 * unusual, so the common path is two clicks, not four.
 */
export function CurrentBlockForm({
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
  const [courseId, setCourseId] = useState("");
  const [blockId, setBlockId] = useState("");

  const [semesters, setSemesters] = useState<Semester[]>([]);
  const [courses, setCourses] = useState<Course[]>([]);
  const [blocks, setBlocks] = useState<Block[]>([]);

  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [fieldErrors, setFieldErrors] = useState<Record<string, string>>({});
  const [saved, setSaved] = useState(false);

  // Each level clears the ones below it, so a stale child id can never be
  // submitted against a freshly chosen parent.
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      setCourses([]);
      setBlocks([]);
      if (!programId) {
        setSemesters([]);
        return;
      }
      try {
        const rows = await listSemesters(programId);
        if (cancelled) return;
        setSemesters(rows);
        setSemesterId((current) => (rows.some((row) => row.id === current) ? current : ""));
      } catch {
        if (!cancelled) setSemesters([]);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [programId]);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      setBlocks([]);
      setCourseId("");
      setBlockId("");
      if (!semesterId) {
        setCourses([]);
        return;
      }
      try {
        const rows = await listCourses(semesterId);
        if (!cancelled) setCourses(rows);
      } catch {
        if (!cancelled) setCourses([]);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [semesterId]);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      setBlockId("");
      if (!courseId) {
        setBlocks([]);
        return;
      }
      try {
        const rows = await listBlocks(courseId);
        if (!cancelled) setBlocks(rows);
      } catch {
        if (!cancelled) setBlocks([]);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [courseId]);

  async function onSubmit(event: React.FormEvent) {
    event.preventDefault();
    setSaved(false);

    if (!blockId) {
      setFieldErrors({ block_id: "Choose a block." });
      setError(null);
      return;
    }

    setSaving(true);
    setError(null);
    setFieldErrors({});
    try {
      await setStudentBlock(student.id, blockId);
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
    <section aria-labelledby="block-heading" className={`${panelClass} p-5`}>
      <h2 id="block-heading" className="font-display text-base font-semibold text-gray-900">
        Set current block
      </h2>
      <p className="mt-1 text-sm text-gray-500">
        {student.current_block_id
          ? "This student already has a block; choosing another replaces it."
          : "This student has no block yet, so their classroom has nothing to teach from."}
      </p>

      <form onSubmit={onSubmit} className="mt-4 space-y-4" noValidate>
        {error ? (
          <Banner tone="danger" onDismiss={() => setError(null)}>
            {error}
          </Banner>
        ) : null}
        {saved ? (
          <Banner tone="success" onDismiss={() => setSaved(false)}>
            Current block updated.
          </Banner>
        ) : null}

        <div className="grid gap-4 sm:grid-cols-2">
          <AdminField id="block_program_id" label="Program">
            <AdminSelect
              id="block_program_id"
              value={programId}
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

          <AdminField id="block_semester_id" label="Semester">
            <AdminSelect
              id="block_semester_id"
              value={semesterId}
              disabled={!programId}
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

          <AdminField id="block_course_id" label="Course">
            <AdminSelect
              id="block_course_id"
              value={courseId}
              disabled={!semesterId}
              onChange={(event) => setCourseId(event.target.value)}
            >
              <option value="">Select a course…</option>
              {courses.map((course) => (
                <option key={course.id} value={course.id}>
                  {course.code} — {course.name}
                </option>
              ))}
            </AdminSelect>
          </AdminField>

          <AdminField id="block_id" label="Block" required error={fieldErrors.block_id}>
            <AdminSelect
              id="block_id"
              value={blockId}
              disabled={!courseId}
              hasError={Boolean(fieldErrors.block_id)}
              onChange={(event) => setBlockId(event.target.value)}
            >
              <option value="">Select a block…</option>
              {blocks.map((block) => (
                <option key={block.id} value={block.id}>
                  {block.block_no} — {block.title}
                </option>
              ))}
            </AdminSelect>
          </AdminField>
        </div>

        <AdminButton type="submit" pending={saving} pendingLabel="Saving…" disabled={!blockId}>
          Set current block
        </AdminButton>
      </form>
    </section>
  );
}
