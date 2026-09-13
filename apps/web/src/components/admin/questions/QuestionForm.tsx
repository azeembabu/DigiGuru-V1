"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useState } from "react";

import { AdminButton, AdminField, AdminInput, AdminSelect, AdminTextarea } from "@/components/admin/controls";
import { Banner, PageHeader, panelClass } from "@/components/admin/primitives";
import { fieldErrorsFor, messageFor } from "@/components/admin/academic/shared";
import { useCascade } from "@/components/admin/questions/cascade";
import { createPoolQuestion } from "@/lib/admin/client";
import {
  ASSESSMENT_TYPES,
  DIFFICULTY_LEVELS,
  type AssessmentType,
  type DifficultyLevel,
} from "@/lib/admin/types";

// Manual single-question entry (A1.3, channel one).
//
// AMENDMENT A2.1: a question belongs to a COURSE. The unit/module is an optional
// last step, so the form requires Program -> Semester -> Course and treats the
// unit as a refinement — leaving it blank pools the question for the whole
// course, which is the normal case.
//
// Exactly four options, one correct, and a required explanation — the
// explanation is NOT NULL in the schema because it is what the student reads on
// the results screen, so the form treats a blank one as an error rather than
// letting the gateway reject it after the typing is done.
//
// Client-side checks here are a courtesy, never the authority: the gateway
// re-validates everything and its message wins. Each rule below is no stricter
// than the server's.

const LETTERS = ["A", "B", "C", "D"] as const;

type Draft = {
  programId: string;
  semesterId: string;
  courseId: string;
  blockId: string;
  topic: string;
  questionText: string;
  options: string[];
  correctIndex: number;
  explanation: string;
  assessmentType: AssessmentType;
  difficulty: DifficultyLevel;
};

const EMPTY: Draft = {
  programId: "",
  semesterId: "",
  courseId: "",
  blockId: "",
  topic: "",
  questionText: "",
  options: ["", "", "", ""],
  correctIndex: 0,
  explanation: "",
  assessmentType: "assignment",
  difficulty: "beginner",
};

function validate(draft: Draft): Record<string, string> {
  const errors: Record<string, string> = {};
  // The course is the required link now; `blockId` is deliberately unchecked.
  if (!draft.courseId) errors.course_id = "Choose the course this question belongs to.";
  if (draft.topic.trim().length < 2) errors.topic = "A topic of at least 2 characters drives the weak-area tagging.";
  if (draft.questionText.trim().length < 5) errors.question_text = "Write the question.";
  if (draft.explanation.trim().length < 5) {
    errors.explanation = "The explanation is shown to the student after they submit, so it is required.";
  }

  draft.options.forEach((option, index) => {
    if (option.trim().length === 0) errors[`options.${index}`] = `Option ${LETTERS[index]} is empty.`;
  });

  // Duplicate options make the correct answer ambiguous even when the index is
  // valid, so this is caught here rather than being a puzzle for the student.
  const seen = new Map<string, number>();
  draft.options.forEach((option, index) => {
    const key = option.trim().toLowerCase();
    if (!key) return;
    const first = seen.get(key);
    if (first === undefined) seen.set(key, index);
    else errors[`options.${index}`] = `Same as option ${LETTERS[first]}.`;
  });

  return errors;
}

export function QuestionForm() {
  const router = useRouter();
  const [draft, setDraft] = useState<Draft>(EMPTY);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [banner, setBanner] = useState<{ tone: "success" | "danger"; text: string } | null>(null);
  const [pending, setPending] = useState(false);

  const cascade = useCascade({
    programId: draft.programId,
    semesterId: draft.semesterId,
    courseId: draft.courseId,
  });

  function patch(next: Partial<Draft>) {
    setDraft((current) => ({ ...current, ...next }));
  }

  function setOption(index: number, value: string) {
    setDraft((current) => {
      const options = [...current.options];
      options[index] = value;
      return { ...current, options };
    });
  }

  async function onSubmit(event: React.FormEvent) {
    event.preventDefault();
    if (pending) return;

    const local = validate(draft);
    setErrors(local);
    if (Object.keys(local).length > 0) {
      setBanner({ tone: "danger", text: "Fix the highlighted fields before saving." });
      return;
    }

    setPending(true);
    setBanner(null);

    try {
      await createPoolQuestion({
        course_id: draft.courseId,
        // Omitted rather than sent empty when no unit was picked: the field is
        // nullable on the server and "" is not a uuid.
        ...(draft.blockId ? { block_id: draft.blockId } : {}),
        topic: draft.topic.trim(),
        question_text: draft.questionText.trim(),
        options: draft.options.map((option) => option.trim()),
        correct_option_index: draft.correctIndex,
        explanation: draft.explanation.trim(),
        assessment_type: draft.assessmentType,
        difficulty_level: draft.difficulty,
      });

      // Keep the placement and the type, clear the question. Questions are
      // entered in runs, and re-picking program/semester/course/block for every
      // one of twenty is the whole cost of manual entry.
      setDraft((current) => ({
        ...current,
        topic: current.topic,
        questionText: "",
        options: ["", "", "", ""],
        correctIndex: 0,
        explanation: "",
      }));
      setErrors({});
      setBanner({ tone: "success", text: "Question added. The form is ready for the next one." });
    } catch (caught: unknown) {
      setErrors(fieldErrorsFor(caught));
      setBanner({ tone: "danger", text: messageFor(caught, "Could not save the question.") });
    } finally {
      setPending(false);
    }
  }

  return (
    <div className="space-y-6">
      <PageHeader
        title="Add a question"
        description="One multiple-choice question with four options, a correct answer, and the rationale the student sees after submitting. It is pooled for its course; linking it to a unit is optional."
        action={
          <Link
            href="/admin/question-pool"
            className="text-sm font-medium text-indigo-500 underline-offset-2 hover:underline"
          >
            Back to the pool
          </Link>
        }
      />

      {banner ? (
        <Banner tone={banner.tone} onDismiss={() => setBanner(null)}>
          {banner.text}
        </Banner>
      ) : null}

      <form onSubmit={onSubmit} className={`${panelClass} space-y-6 p-5`} noValidate>
        <fieldset className="space-y-4">
          <legend className="text-sm font-semibold text-gray-900">Placement</legend>
          <p className="text-xs text-gray-500">
            A question is pooled per <strong>course</strong> — that is what an exam paper is sampled
            from. The unit/module is optional: set it only to record which part of the course the
            question covers.
          </p>

          <div className="grid gap-4 sm:grid-cols-2">
            <AdminField id="program_id" label="Program" required error={errors.program_id}>
              <AdminSelect
                id="program_id"
                value={draft.programId}
                hasError={Boolean(errors.program_id)}
                onChange={(e) =>
                  patch({ programId: e.target.value, semesterId: "", courseId: "", blockId: "" })
                }
              >
                <option value="">Choose a program…</option>
                {cascade.programs.map((program) => (
                  <option key={program.id} value={program.id}>
                    {program.code} — {program.name}
                  </option>
                ))}
              </AdminSelect>
            </AdminField>

            <AdminField id="semester_id" label="Semester" required>
              <AdminSelect
                id="semester_id"
                value={draft.semesterId}
                disabled={!draft.programId}
                onChange={(e) => patch({ semesterId: e.target.value, courseId: "", blockId: "" })}
              >
                <option value="">
                  {draft.programId ? "Choose a semester…" : "Pick a program first"}
                </option>
                {cascade.semesters.map((semester) => (
                  <option key={semester.id} value={semester.id}>
                    Semester {semester.semester_number}
                  </option>
                ))}
              </AdminSelect>
            </AdminField>

            <AdminField id="course_id" label="Course" required error={errors.course_id}>
              <AdminSelect
                id="course_id"
                value={draft.courseId}
                hasError={Boolean(errors.course_id)}
                disabled={!draft.semesterId}
                onChange={(e) => patch({ courseId: e.target.value, blockId: "" })}
              >
                <option value="">
                  {draft.semesterId ? "Choose a course…" : "Pick a semester first"}
                </option>
                {cascade.courses.map((course) => (
                  <option key={course.id} value={course.id}>
                    {course.code} — {course.name}
                  </option>
                ))}
              </AdminSelect>
            </AdminField>

            <AdminField
              id="block_id"
              label="Unit or module"
              error={errors.block_id}
              hint="Optional. Leave it unset to pool the question for the whole course."
            >
              <AdminSelect
                id="block_id"
                value={draft.blockId}
                hasError={Boolean(errors.block_id)}
                hasHint
                disabled={!draft.courseId}
                onChange={(e) => patch({ blockId: e.target.value })}
              >
                <option value="">
                  {draft.courseId ? "No specific unit" : "Pick a course first"}
                </option>
                {cascade.blocks.map((block) => (
                  <option key={block.id} value={block.id}>
                    Unit {block.block_no} — {block.title}
                  </option>
                ))}
              </AdminSelect>
            </AdminField>
          </div>
        </fieldset>

        <fieldset className="space-y-4 border-t border-lavender-200 pt-5">
          <legend className="text-sm font-semibold text-gray-900">Classification</legend>
          <div className="grid gap-4 sm:grid-cols-3">
            <AdminField id="assessment_type" label="Assessment type" required error={errors.assessment_type}>
              <AdminSelect
                id="assessment_type"
                value={draft.assessmentType}
                hasError={Boolean(errors.assessment_type)}
                onChange={(e) => patch({ assessmentType: e.target.value as AssessmentType })}
              >
                {ASSESSMENT_TYPES.map((type) => (
                  <option key={type.value} value={type.value}>
                    {type.label}
                  </option>
                ))}
              </AdminSelect>
            </AdminField>

            <AdminField id="difficulty_level" label="Difficulty" required error={errors.difficulty_level}>
              <AdminSelect
                id="difficulty_level"
                value={draft.difficulty}
                hasError={Boolean(errors.difficulty_level)}
                onChange={(e) => patch({ difficulty: e.target.value as DifficultyLevel })}
              >
                {DIFFICULTY_LEVELS.map((level) => (
                  <option key={level.value} value={level.value}>
                    {level.label}
                  </option>
                ))}
              </AdminSelect>
            </AdminField>

            <AdminField
              id="topic"
              label="Topic"
              required
              error={errors.topic}
              hint="Shown to the student as a weak area when they get this wrong."
            >
              <AdminInput
                id="topic"
                value={draft.topic}
                maxLength={200}
                hasError={Boolean(errors.topic)}
                hasHint
                onChange={(e) => patch({ topic: e.target.value })}
              />
            </AdminField>
          </div>
        </fieldset>

        <fieldset className="space-y-4 border-t border-lavender-200 pt-5">
          <legend className="text-sm font-semibold text-gray-900">The question</legend>

          <AdminField id="question_text" label="Question" required error={errors.question_text}>
            <AdminTextarea
              id="question_text"
              value={draft.questionText}
              rows={3}
              hasError={Boolean(errors.question_text)}
              onChange={(e) => patch({ questionText: e.target.value })}
            />
          </AdminField>

          <div>
            <p className="text-sm font-medium text-gray-900">
              Options
              <span className="ml-0.5 text-danger" aria-hidden="true">
                *
              </span>
            </p>
            <p className="mt-1 text-xs text-gray-500">
              Exactly four. Select the radio beside the correct one — that is the answer key, and it
              is never sent to a student before they submit.
            </p>

            <div className="mt-3 space-y-2.5">
              {draft.options.map((option, index) => {
                const id = `option-${index}`;
                const error = errors[`options.${index}`];
                return (
                  <div key={index} className="flex items-start gap-3">
                    <label className="mt-2 flex shrink-0 items-center gap-2">
                      <input
                        type="radio"
                        name="correct_option_index"
                        value={index}
                        checked={draft.correctIndex === index}
                        onChange={() => patch({ correctIndex: index })}
                        className="h-4 w-4 accent-lime-500"
                      />
                      <span className="text-sm font-semibold text-gray-900">{LETTERS[index]}</span>
                      <span className="sr-only">Mark option {LETTERS[index]} as correct</span>
                    </label>
                    <div className="min-w-0 flex-1">
                      <label htmlFor={id} className="sr-only">
                        Option {LETTERS[index]}
                      </label>
                      <AdminInput
                        id={id}
                        value={option}
                        maxLength={500}
                        hasError={Boolean(error)}
                        placeholder={`Option ${LETTERS[index]}`}
                        onChange={(e) => setOption(index, e.target.value)}
                      />
                      {error ? (
                        <p role="alert" className="mt-1 text-xs text-danger">
                          {error}
                        </p>
                      ) : null}
                    </div>
                  </div>
                );
              })}
            </div>

            {errors.correct_option_index ? (
              <p role="alert" className="mt-2 text-xs text-danger">
                {errors.correct_option_index}
              </p>
            ) : null}
          </div>

          <AdminField
            id="explanation"
            label="Explanation"
            required
            error={errors.explanation}
            hint="Why the correct option is correct. This is the remediation text on the student's results screen."
          >
            <AdminTextarea
              id="explanation"
              value={draft.explanation}
              rows={3}
              hasError={Boolean(errors.explanation)}
              hasHint
              onChange={(e) => patch({ explanation: e.target.value })}
            />
          </AdminField>
        </fieldset>

        <div className="flex flex-wrap gap-3 border-t border-lavender-200 pt-5">
          <AdminButton type="submit" pending={pending} pendingLabel="Saving…">
            Save question
          </AdminButton>
          <AdminButton
            type="button"
            variant="outline-light"
            onClick={() => router.push("/admin/question-pool")}
          >
            Done
          </AdminButton>
        </div>
      </form>
    </div>
  );
}
