"use client";

import { useState } from "react";

import { AdminField, AdminInput, AdminSelect, AdminTextarea } from "@/components/admin/controls";
import { FormModal } from "@/components/admin/academic/FormModal";
import {
  fieldErrorsFor,
  lengthRule,
  messageFor,
  optionalText,
} from "@/components/admin/academic/shared";
import { createCourse, updateCourse } from "@/lib/admin/client";
import type { Course, Semester } from "@/lib/admin/types";

/**
 * Create/edit a course.
 *
 * Semester is flattened out of the *navigation* but not out of the data: the
 * create endpoint still requires a real `semester_id`, and `students.semester_id`
 * and the Qdrant metadata filter still key off it. So the field stays, as a
 * select on create.
 *
 * It is absent on edit because `UpdateCourseRequest` carries only `name` and
 * `description` — moving a course between semesters is not something the API
 * can do, and a select that silently discarded the change would be a lie.
 */
export function CourseForm({
  programId,
  semesters,
  course,
  defaultSemesterId,
  onClose,
  onSaved,
}: {
  programId: string;
  /** Every semester of this program, for the create-mode select. */
  semesters: Semester[];
  course: Course | null;
  /** Preselected semester, when the form was opened from one. */
  defaultSemesterId?: string;
  onClose: () => void;
  onSaved: (message: string) => void;
}) {
  const [semesterId, setSemesterId] = useState(
    defaultSemesterId ?? course?.semester_id ?? semesters[0]?.id ?? "",
  );
  const [code, setCode] = useState(course?.code ?? "");
  const [name, setName] = useState(course?.name ?? "");
  const [description, setDescription] = useState(course?.description ?? "");
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [formError, setFormError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);

  function validate(): Record<string, string> {
    const next: Record<string, string> = {};
    if (!course) {
      if (semesterId === "") next.semester_id = "Choose a semester.";
      const codeError = lengthRule(code, 2, 20, "Code");
      if (codeError) next.code = codeError;
    }
    const nameError = lengthRule(name, 2, 200, "Name");
    if (nameError) next.name = nameError;
    return next;
  }

  async function submit() {
    const found = validate();
    setErrors(found);
    if (Object.keys(found).length > 0) return;

    setPending(true);
    setFormError(null);
    try {
      if (course) {
        await updateCourse(course.id, {
          name: name.trim(),
          description: optionalText(description),
        });
        onSaved(`Course “${name.trim()}” updated.`);
      } else {
        await createCourse({
          program_id: programId,
          semester_id: semesterId,
          code: code.trim(),
          name: name.trim(),
          description: optionalText(description),
        });
        onSaved(`Course “${name.trim()}” created.`);
      }
    } catch (caught: unknown) {
      setErrors(fieldErrorsFor(caught));
      setFormError(messageFor(caught, "Could not save the course."));
    } finally {
      setPending(false);
    }
  }

  const noSemesters = !course && semesters.length === 0;

  return (
    <FormModal
      open
      title={course ? "Edit course" : "Add new course"}
      description={
        course
          ? "The course code and semester are fixed once the course exists."
          : "A course belongs to one semester of this program."
      }
      submitLabel={course ? "Save changes" : "Create course"}
      pending={pending}
      error={
        noSemesters
          ? "This program has no semesters yet, and a course must belong to one. Add a semester first."
          : formError
      }
      onSubmit={submit}
      onClose={onClose}
    >
      {course ? null : (
        <AdminField id="course-semester" label="Semester" required error={errors.semester_id}>
          <AdminSelect
            id="course-semester"
            value={semesterId}
            onChange={(event) => setSemesterId(event.target.value)}
            hasError={Boolean(errors.semester_id)}
            disabled={semesters.length === 0}
          >
            {semesters.length === 0 ? <option value="">No semesters yet</option> : null}
            {semesters.map((semester) => (
              <option key={semester.id} value={semester.id}>
                {semester.semester_number}. {semester.name}
              </option>
            ))}
          </AdminSelect>
        </AdminField>
      )}

      {course ? null : (
        <AdminField id="course-code" label="Code" required error={errors.code} hint="2–20 characters.">
          <AdminInput
            id="course-code"
            value={code}
            onChange={(event) => setCode(event.target.value)}
            hasError={Boolean(errors.code)}
            hasHint
            autoComplete="off"
            placeholder="ML101"
          />
        </AdminField>
      )}

      <AdminField id="course-name" label="Name" required error={errors.name}>
        <AdminInput
          id="course-name"
          value={name}
          onChange={(event) => setName(event.target.value)}
          hasError={Boolean(errors.name)}
          autoComplete="off"
          placeholder="Malayalam Prose"
        />
      </AdminField>

      <AdminField id="course-description" label="Description" error={errors.description}>
        <AdminTextarea
          id="course-description"
          value={description}
          onChange={(event) => setDescription(event.target.value)}
          hasError={Boolean(errors.description)}
          placeholder="Optional description…"
        />
      </AdminField>
    </FormModal>
  );
}
