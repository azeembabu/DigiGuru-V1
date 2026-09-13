"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useCallback, useEffect, useMemo, useState } from "react";

import { ApiError } from "@/lib/api";
import {
  SchemaError,
  loadExamAttempts,
  loadStudentDashboard,
  loadStudentExams,
  type ExamAttemptRow,
  type StudentDashboard,
  type StudentExam,
} from "@/lib/student";
import {
  assessmentProgress,
  attentionItems,
  courseProgress,
  courseRows,
  formatDateTime,
  performanceSeries,
  revisionBacklog,
  speakingTime,
  weakTopics,
  type CourseRow,
} from "@/lib/dashboard-metrics";
import { AnswerSheetPanel } from "./AnswerSheetPanel";
import { CourseTable } from "./CourseTable";
import { KpiRow } from "./KpiRow";
import { LuxHeader } from "./LuxHeader";
import { LuxSidebar, type CourseOption } from "./LuxSidebar";
import { PerformanceChart } from "./PerformanceChart";
import { WeakTopicPanel } from "./WeakTopicPanel";
import { Badge, Eyebrow, SectionHeading, Skeleton, SubText, lightCard, luxButton } from "./shell";
import { VizEmpty } from "./viz";

// The quiet-luxury student dashboard: dark charcoal rail, light cream content.
//
// Client-rendered rather than fetched in a Server Component, for the reason
// documented at the top of `lib/student-context.ts`: auth is an httpOnly cookie
// the gateway sets on its own host, and a Server Component would have to
// re-forward it by hand on every call.
//
// Three payloads feed the page, and they fail independently on purpose:
//
//   `GET /student/dashboard`      essential — its failure is the page's failure.
//   `GET /student/exams`          the table and the "attempted" tile.
//   `GET /student/exam-attempts`  the chart and the per-course trendlines.
//
// A failed secondary list degrades its own section to a "could not load" state
// and leaves the rest of the page intact. It never renders as "you have nothing",
// which would tell a student their record is empty when it is merely unread.

type Essential =
  | { status: "loading" }
  | { status: "error"; message: string; retryable: boolean }
  | { status: "ready"; data: StudentDashboard };

/** A secondary list: loaded, failed, or still arriving. Failure is not emptiness. */
type Aux<T> = { status: "loading" } | { status: "error" } | { status: "ready"; items: T[] };

/**
 * A result tagged with the attempt it answers.
 *
 * "Still loading" is then a comparison during render rather than a `setState` in
 * the effect body, which is both a cascading render and a lint error. It is also
 * what makes a retry correct: a slow response from attempt N cannot overwrite the
 * fresh state of attempt N+1, because the render only trusts a result whose tag
 * matches the current attempt. The same shape `StudentOverview` and the admin
 * console's `useResource` already use.
 */
type Tagged<T> = { attempt: number; value: T };

export function LuxDashboard() {
  const router = useRouter();
  const [attempt, setAttempt] = useState(0);
  const [essentialState, setEssential] = useState<Tagged<Essential> | null>(null);
  const [examsState, setExams] = useState<Tagged<Aux<StudentExam>> | null>(null);
  const [attemptsState, setAttempts] = useState<Tagged<Aux<ExamAttemptRow>> | null>(null);
  // The clock, read after mount rather than during render — a render-time
  // `Date.now()` is a hydration mismatch, and the timeframe filter is the only
  // thing that needs it. Set from a timeout so it is not a synchronous setState
  // in an effect body either.
  const [nowMs, setNowMs] = useState(0);

  const [search, setSearch] = useState("");
  const [courseFilter, setCourseFilter] = useState("all");

  useEffect(() => {
    const timer = setTimeout(() => setNowMs(Date.now()), 0);
    return () => clearTimeout(timer);
  }, [attempt]);

  useEffect(() => {
    let cancelled = false;
    const current = attempt;

    loadStudentDashboard()
      .then((data) => {
        if (!cancelled) setEssential({ attempt: current, value: { status: "ready", data } });
      })
      .catch((caught: unknown) => {
        if (cancelled) return;

        // Only the gateway can judge the cookie, so its 401 drives the bounce.
        if (caught instanceof ApiError && caught.status === 401) {
          router.replace("/login");
          return;
        }

        // A 404 is the gateway saying "you are not a student": these routes are
        // self-only and resolve the subject from the token, so an admin signed
        // into this page legitimately has no dashboard. Documented behaviour,
        // not something to retry.
        if (caught instanceof ApiError && caught.status === 404) {
          setEssential({
            attempt: current,
            value: {
              status: "error",
              message:
                "This account has no student record, so there is no study dashboard to show. Administrator accounts use the console instead.",
              retryable: false,
            },
          });
          return;
        }

        setEssential({
          attempt: current,
          value: {
            status: "error",
            message:
              caught instanceof SchemaError || caught instanceof ApiError
                ? caught.message
                : "Your dashboard could not be loaded. Please try again.",
            // A schema mismatch will not fix itself on a retry — it is a bug on
            // our side, and offering "try again" for it wastes the student's time.
            retryable: !(caught instanceof SchemaError),
          },
        });
      });

    loadStudentExams({ limit: 200 })
      .then(({ items }) => {
        if (!cancelled) setExams({ attempt: current, value: { status: "ready", items } });
      })
      .catch(() => {
        if (!cancelled) setExams({ attempt: current, value: { status: "error" } });
      });

    loadExamAttempts({ limit: 200 })
      .then(({ items }) => {
        if (!cancelled) setAttempts({ attempt: current, value: { status: "ready", items } });
      })
      .catch(() => {
        if (!cancelled) setAttempts({ attempt: current, value: { status: "error" } });
      });

    return () => {
      cancelled = true;
    };
  }, [router, attempt]);

  const retry = useCallback(() => setAttempt((n) => n + 1), []);

  // ---------------------------------------------------------------- derived

  // A result left over from an earlier attempt reads as "still loading", which
  // is exactly what it is from the current attempt's point of view.
  //
  // Memoised, not a bare ternary: the `{ status: "loading" }` fallback would be a
  // new object on every render, which invalidates every `useMemo` downstream of
  // it and re-derives the whole page on each keystroke in the search box.
  const essential = useMemo<Essential>(
    () =>
      essentialState !== null && essentialState.attempt === attempt
        ? essentialState.value
        : { status: "loading" },
    [essentialState, attempt],
  );
  const exams = useMemo<Aux<StudentExam>>(
    () =>
      examsState !== null && examsState.attempt === attempt
        ? examsState.value
        : { status: "loading" },
    [examsState, attempt],
  );
  const attempts = useMemo<Aux<ExamAttemptRow>>(
    () =>
      attemptsState !== null && attemptsState.attempt === attempt
        ? attemptsState.value
        : { status: "loading" },
    [attemptsState, attempt],
  );

  // Memoised so the identity is stable between renders. A fresh `[]` each render
  // would invalidate every downstream `useMemo` and re-derive the whole page on
  // every keystroke in the search box.
  const examItems = useMemo(() => (exams.status === "ready" ? exams.items : []), [exams]);
  const attemptItems = useMemo(
    () => (attempts.status === "ready" ? attempts.items : []),
    [attempts],
  );

  const courses = useMemo<CourseOption[]>(() => {
    const byId = new Map<string, CourseOption>();
    for (const exam of examItems) {
      byId.set(exam.course_id, {
        id: exam.course_id,
        code: exam.course_code,
        name: exam.course_name,
      });
    }
    return [...byId.values()].sort((a, b) => a.code.localeCompare(b.code));
  }, [examItems]);

  // The workspace selector filters the chart and the table against real rows.
  const scopedExams = useMemo(
    () => (courseFilter === "all" ? examItems : examItems.filter((e) => e.course_id === courseFilter)),
    [examItems, courseFilter],
  );
  const scopedAttempts = useMemo(
    () =>
      courseFilter === "all"
        ? attemptItems
        : attemptItems.filter((a) => a.course_id === courseFilter),
    [attemptItems, courseFilter],
  );

  const series = useMemo(() => performanceSeries(scopedAttempts), [scopedAttempts]);
  const allRows = useMemo(() => courseRows(scopedExams, scopedAttempts), [scopedExams, scopedAttempts]);

  const needle = search.trim().toLowerCase();
  const rows = useMemo<CourseRow[]>(
    () =>
      needle === ""
        ? allRows
        : allRows.filter(
            (row) =>
              row.courseCode.toLowerCase().includes(needle) ||
              row.courseName.toLowerCase().includes(needle),
          ),
    [allRows, needle],
  );

  // Derived before the early returns, not after: a `useMemo` below a conditional
  // `return` runs on some renders and not others, which is the hooks-order bug
  // React cannot recover from. The empty-array fallback is only ever read while
  // the payload is absent, and nothing renders from it in that state.
  const topics = useMemo(() => {
    const history = essential.status === "ready" ? essential.data.exam_history : [];
    const all = weakTopics(history);
    return needle === "" ? all : all.filter((t) => t.topic.toLowerCase().includes(needle));
  }, [essential, needle]);

  if (essential.status === "loading") {
    return (
      <DashboardFrame>
        <LoadingBody />
      </DashboardFrame>
    );
  }

  if (essential.status === "error") {
    return (
      <DashboardFrame>
        <div className="mx-auto max-w-lg py-10">
          <VizEmpty
            kind="error"
            title="Your dashboard could not be loaded"
            hint={essential.message}
            action={
              essential.retryable ? (
                <button type="button" onClick={retry} className={luxButton("primary")}>
                  Try again
                </button>
              ) : undefined
            }
          />
        </div>
      </DashboardFrame>
    );
  }

  const { student_info, continue_learning, quota_status, exam_history, saved_resources } =
    essential.data;

  const speaking = speakingTime(quota_status);
  const backlog = revisionBacklog(saved_resources);
  const progress = assessmentProgress(examItems);

  const selectedCourse = courses.find((c) => c.id === courseFilter) ?? null;

  // The CTA follows the real state rather than always saying "resume": when the
  // daily voice quota is spent, the classroom cannot give the student a session,
  // so sending them there would be a dead end.
  const cta = speaking.isLocked
    ? {
        href: "/exams",
        label: "Practise assessments",
        hint: "Today’s speaking time is used — assessments and notes are still open.",
      }
    : continue_learning === null
      ? {
          href: "/classroom",
          label: "Start learning",
          hint: `${student_info.program} · Semester ${student_info.semester}`,
        }
      : {
          href: "/classroom",
          label: "Resume session",
          hint: `Block ${continue_learning.block_no} · ${continue_learning.block_title}`,
        };

  return (
    <DashboardFrame
      sidebar={
        <LuxSidebar
          courses={courses}
          courseFilter={courseFilter}
          onCourseFilter={setCourseFilter}
          search={search}
          onSearch={setSearch}
          name={student_info.name}
          rollNumber={student_info.roll_number}
          programme={`Sem ${student_info.semester}`}
        />
      }
      header={<LuxHeader items={attentionItems({ backlog, speaking, progress, ungradedAttempts: series.ungradedCount })} cta={cta} />}
    >
      <div className="space-y-5">
        <ResumeStrip resume={continue_learning} />

        <KpiRow
          speaking={speaking}
          resetsAt={quota_status.resets_at}
          timezone={quota_status.timezone}
          backlog={backlog}
          progress={progress}
          courseProgressValue={courseProgress(continue_learning)}
          courseTitle={continue_learning?.course_title ?? null}
        />

        {/* A failed secondary list says so, in its own place, with a retry —
            never a zeroed chart. */}
        {attempts.status === "error" ? (
          <VizEmpty
            kind="error"
            title="Your assessment scores could not be loaded"
            hint="The rest of your dashboard is up to date. This section alone failed to load."
            action={
              <button type="button" onClick={retry} className={luxButton("quiet")}>
                Try again
              </button>
            }
          />
        ) : attempts.status === "loading" ? (
          <Skeleton className="h-[340px]" />
        ) : (
          <PerformanceChart
            series={series}
            nowMs={nowMs}
            courseLabel={selectedCourse === null ? null : `${selectedCourse.code} ${selectedCourse.name}`}
          />
        )}

        {/* `min-w-0` on both grid children is load-bearing, not tidying: a grid
            item's default `min-width: auto` is its content's minimum, so the
            table's `min-w-[620px]` would widen the whole track and push the PAGE
            into horizontal scroll at 400px — the overflow container inside it
            cannot shrink below a parent that refuses to. Measured: 678px of
            scroll width in a 400px viewport before this. */}
        <div className="grid gap-5 xl:grid-cols-3">
          <div className="min-w-0 xl:col-span-2">
            {exams.status === "error" ? (
              <VizEmpty
                kind="error"
                title="Your course list could not be loaded"
                hint="This section alone failed to load; nothing else on the page is affected."
                action={
                  <button type="button" onClick={retry} className={luxButton("quiet")}>
                    Try again
                  </button>
                }
              />
            ) : exams.status === "loading" ? (
              <Skeleton className="h-[280px]" />
            ) : (
              <CourseTable rows={rows} filter={search} totalRowCount={allRows.length} />
            )}
          </div>

          <WeakTopicPanel
            topics={topics}
            attemptCount={exam_history.filter((e) => e.percentage !== null).length}
            filter={search}
          />
        </div>

        {/*
          Full width, under the two-column row: an answer sheet is a document
          the student goes looking for deliberately, so it gets a labelled
          section of its own rather than a link buried in the chart.
        */}
        <div className="mt-6">
          {attempts.status === "ready" ? (
            <AnswerSheetPanel attempts={attemptItems} />
          ) : attempts.status === "loading" ? (
            <Skeleton className="h-[180px]" />
          ) : null}
        </div>
      </div>
    </DashboardFrame>
  );
}

// ------------------------------------------------------------------- frame

/**
 * The two-surface frame.
 *
 * The rail is fixed, so the content column carries its own left offset rather
 * than living in a flex row — that keeps the sticky header aligned with the
 * content instead of spanning the rail. Below `lg` the rail collapses entirely
 * into its own drawer and the offset disappears, which is what keeps the 400px
 * width free of horizontal scroll.
 */
function DashboardFrame({
  sidebar,
  header,
  children,
}: {
  sidebar?: React.ReactNode;
  header?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <div className="min-h-dvh bg-lux-cream-100 text-lux-ink-900">
      {sidebar}
      <div className="lg:pl-[264px]">
        {header}
        <main className="mx-auto w-full max-w-[1200px] px-4 py-6 sm:px-6 sm:py-8">{children}</main>
      </div>
    </div>
  );
}

function LoadingBody() {
  return (
    <div className="space-y-5" aria-busy="true" aria-label="Loading your dashboard">
      <Skeleton className="h-20" />
      <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
        <Skeleton className="h-44" />
        <Skeleton className="h-44" />
        <Skeleton className="h-44" />
        <Skeleton className="h-44" />
      </div>
      <Skeleton className="h-[340px]" />
      <div className="grid gap-5 xl:grid-cols-3">
        <Skeleton className="h-[280px] xl:col-span-2" />
        <Skeleton className="h-[280px]" />
      </div>
    </div>
  );
}

// -------------------------------------------------------------- resume strip

/**
 * Where the student stopped.
 *
 * `chapter_name`, `para_index` and `resume_summary` are all genuinely null in
 * practice (they are only present when the Redis session state carried them), so
 * each is rendered only when it exists rather than as "Chapter null". `page_number`
 * is likewise omitted at zero, which means "no page recorded" here rather than
 * "page zero".
 */
function ResumeStrip({ resume }: { resume: StudentDashboard["continue_learning"] }) {
  if (resume === null) {
    return (
      <section aria-labelledby="resume-heading" className={`${lightCard} p-5`}>
        <Eyebrow>Continue learning</Eyebrow>
        <SectionHeading id="resume-heading">No session yet</SectionHeading>
        <div className="mt-1">
          <SubText>
            Your first classroom session starts with a short introduction, then picks up wherever
            you stop.
          </SubText>
        </div>
        <div className="mt-4">
          <Link href="/classroom" className={luxButton("primary")}>
            Open the classroom
          </Link>
        </div>
      </section>
    );
  }

  return (
    <section
      aria-labelledby="resume-heading"
      className={`${lightCard} flex flex-wrap items-center justify-between gap-4 p-5`}
    >
      <div className="min-w-0">
        <Eyebrow>Continue learning</Eyebrow>
        <SectionHeading id="resume-heading">
          Block {resume.block_no} · {resume.block_title}
        </SectionHeading>
        <div className="mt-1 flex flex-wrap items-center gap-x-3 gap-y-1">
          <SubText>{resume.course_title}</SubText>
          {resume.chapter_name !== null ? <SubText>Chapter {resume.chapter_name}</SubText> : null}
          {resume.page_number > 0 ? <SubText>Page {resume.page_number}</SubText> : null}
          <Badge tone="neutral">Last active {formatDateTime(resume.last_active_at)}</Badge>
        </div>
        {resume.resume_summary !== null ? (
          <p className="mt-2 max-w-2xl text-[14px] leading-[1.6] text-lux-ink-700">
            {resume.resume_summary}
          </p>
        ) : null}
      </div>

      <Link href="/classroom" className={luxButton("primary", "shrink-0")}>
        Resume
      </Link>
    </section>
  );
}
