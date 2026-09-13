"use client";

import { useCallback } from "react";

import { DataTable, type Column } from "@/components/admin/DataTable";
import { Pagination } from "@/components/admin/controls";
import { Banner, PageHeader, StatusPill } from "@/components/admin/primitives";
import { useResource } from "@/components/admin/academic/shared";
import {
  ActiveFilters,
  Blank,
  FilterBar,
  FilterSelect,
  PAGE_SIZE,
  QUOTA_CAP_MS,
  RowLink,
  ViolationCount,
  formatDateTime,
  formatDuration,
  useUrlFilters,
} from "@/components/admin/drilldown/shared";
import { listSessions } from "@/lib/admin/client";
import type { LearningSession, SessionEndReason, SessionStatus } from "@/lib/admin/types";

const STATUSES: { value: SessionStatus; label: string }[] = [
  { value: "in_progress", label: "In progress" },
  { value: "completed", label: "Completed" },
  { value: "abandoned", label: "Abandoned" },
];

const END_REASONS: { value: SessionEndReason; label: string }[] = [
  { value: "quota", label: "Hit the 20-minute cap" },
  { value: "idle", label: "Idle timeout" },
  { value: "user", label: "Ended by the student" },
  { value: "jailbreak", label: "Safety termination" },
  { value: "error", label: "Error" },
];

const END_REASON_LABEL: Record<SessionEndReason, string> = {
  quota: "20-minute cap",
  idle: "Idle timeout",
  user: "Student ended",
  jailbreak: "Safety termination",
  error: "Error",
};

/**
 * How a filtered view describes itself.
 *
 * The heading has to say what is on screen in words — an admin arriving from a
 * dashboard tile did not type the query string and should not have to read it
 * back off the address bar to know what they are looking at.
 */
function headingFor(status: string, endReason: string): { title: string; description: string } {
  switch (endReason) {
    case "quota":
      return {
        title: "Sessions that hit the 20-minute cap",
        description:
          "The NN-3 quota stopped these sessions at 20 minutes of active voice. The cap is server-authoritative and counts only while voice is live — a client clock never contributes to it.",
      };
    case "jailbreak":
      return {
        title: "Sessions ended by a safety termination",
        description:
          "A Tier-1 guardrail hit tore the socket down (close code 4009) and moved the session to JAILBREAK_HALT.",
      };
    case "idle":
      return {
        title: "Sessions closed on idle timeout",
        description: "No speech for the idle window, so the gateway closed the socket (close code 4008).",
      };
    case "error":
      return {
        title: "Sessions that ended in an error",
        description:
          "An upstream or gateway failure ended these sessions rather than the student or the quota.",
      };
    case "user":
      return {
        title: "Sessions the student ended",
        description: "Closed normally from the classroom.",
      };
    default:
      break;
  }

  switch (status) {
    case "in_progress":
      return {
        title: "Sessions in progress",
        description:
          "Live classroom sessions — voice time is still accruing against the 20-minute cap.",
      };
    case "abandoned":
      return {
        title: "Abandoned sessions",
        description:
          "The socket went away without an explicit end; these never reached a clean close.",
      };
    case "completed":
      return { title: "Completed sessions", description: "Sessions that reached a clean close." };
    default:
      return {
        title: "Learning sessions",
        description:
          "Every classroom session, newest first, with its NN-3 voice time and NN-1 whiteboard record.",
      };
  }
}

function EndReason({ session }: { session: LearningSession }) {
  if (session.end_reason === null) {
    return <StatusPill tone="info">still running</StatusPill>;
  }
  const tone =
    session.end_reason === "jailbreak" || session.end_reason === "error"
      ? "danger"
      : session.end_reason === "quota" || session.end_reason === "idle"
        ? "warning"
        : "neutral";
  return <StatusPill tone={tone}>{END_REASON_LABEL[session.end_reason]}</StatusPill>;
}

/**
 * The sessions drill-down.
 *
 * Filters come from the URL so a dashboard metric can link straight to, say,
 * `?end_reason=quota` and the resulting view stays pasteable into a bug report.
 */
export function SessionsScreen() {
  const { get, offset, set, setOffset, clearAll } = useUrlFilters();
  const status = get("status");
  const endReason = get("end_reason");
  const studentId = get("student_id");
  const blockId = get("block_id");
  const courseId = get("course_id");
  const from = get("from");
  const to = get("to");

  const loader = useCallback(
    () =>
      listSessions({
        status: (status || undefined) as SessionStatus | undefined,
        end_reason: (endReason || undefined) as SessionEndReason | undefined,
        student_id: studentId || undefined,
        block_id: blockId || undefined,
        course_id: courseId || undefined,
        from: from || undefined,
        to: to || undefined,
        limit: PAGE_SIZE,
        offset,
      }),
    [status, endReason, studentId, blockId, courseId, from, to, offset],
  );
  const { data, loading, error } = useResource(loader, "Could not load sessions.");

  const heading = headingFor(status, endReason);
  const active: string[] = [];
  if (status) active.push(STATUSES.find((item) => item.value === status)?.label ?? status);
  if (endReason) {
    active.push(END_REASONS.find((item) => item.value === endReason)?.label ?? endReason);
  }
  if (studentId) active.push("one student");
  if (blockId) active.push("one block");
  if (courseId) active.push("one course");
  if (from || to) active.push("a date range");

  const columns: Column<LearningSession>[] = [
    {
      key: "student",
      header: "Student",
      cell: (row) => (
        <div className="min-w-[10rem]">
          <RowLink href={`/admin/students/${row.student_id}`}>{row.student_name}</RowLink>
          <p className="mt-0.5 font-mono text-xs text-gray-500">{row.roll_number}</p>
        </div>
      ),
    },
    {
      key: "block",
      header: "Block",
      cell: (row) => (
        <div className="min-w-[10rem]">
          <p className="font-medium text-gray-900">
            {row.course_code} · Block {row.block_no}
          </p>
          <p className="mt-0.5 text-xs text-gray-500">{row.block_title}</p>
        </div>
      ),
    },
    {
      key: "started",
      header: "Started",
      cell: (row) => (
        <span className="whitespace-nowrap text-gray-500">{formatDateTime(row.started_at)}</span>
      ),
    },
    {
      key: "voice",
      header: "Voice time",
      align: "right",
      cell: (row) => {
        // The cap is the interesting case: an admin asking "why did this stop"
        // needs to see at a glance that it was the quota, not the student.
        const atCap = row.active_voice_ms >= QUOTA_CAP_MS;
        return (
          <span className="whitespace-nowrap">
            <span className={atCap ? "font-semibold text-[#8a5d05]" : "text-gray-900"}>
              {formatDuration(row.active_voice_ms)}
            </span>
            {atCap ? (
              <span className="ml-2">
                <StatusPill tone="warning">at cap</StatusPill>
              </span>
            ) : null}
          </span>
        );
      },
    },
    {
      key: "end",
      header: "Ended",
      cell: (row) => (
        <div className="min-w-[9rem]">
          <EndReason session={row} />
          {row.ended_at ? (
            <p className="mt-0.5 whitespace-nowrap text-xs text-gray-500">
              {formatDateTime(row.ended_at)}
            </p>
          ) : null}
        </div>
      ),
    },
    {
      key: "topic",
      header: "Last topic",
      className: "max-w-[220px]",
      cell: (row) =>
        row.last_topic ? (
          <span className="line-clamp-2 text-gray-500">
            {row.last_topic}
            {row.last_page > 0 ? ` · p.${row.last_page}` : ""}
          </span>
        ) : (
          <Blank />
        ),
    },
    {
      key: "board",
      header: "Whiteboard",
      align: "right",
      cell: (row) => (
        <div className="flex flex-col items-end gap-1">
          <ViolationCount count={row.board_violations} />
          <RowLink href={`/admin/board-events?session_id=${row.id}`}>
            {row.board_ops} op{row.board_ops === 1 ? "" : "s"}
          </RowLink>
        </div>
      ),
    },
  ];

  return (
    <div className="space-y-6">
      <PageHeader title={heading.title} description={heading.description} />

      <FilterBar>
        <FilterSelect
          id="session-status"
          label="Session status"
          value={status}
          anyLabel="Any status"
          options={STATUSES}
          onChange={(next) => set({ status: next })}
        />
        <FilterSelect
          id="session-end-reason"
          label="How it ended"
          value={endReason}
          anyLabel="Any ending"
          options={END_REASONS}
          onChange={(next) => set({ end_reason: next })}
        />
      </FilterBar>

      <ActiveFilters labels={active} onClear={clearAll} />

      {error ? (
        // Never an empty table: "the query failed" and "there is nothing here"
        // are opposite conclusions for someone diagnosing a problem.
        <Banner tone="danger">{error}</Banner>
      ) : (
        <>
          <DataTable
            columns={columns}
            rows={data?.items ?? []}
            rowKey={(row) => row.id}
            loading={loading}
            empty="No sessions match these filters."
            emptyHint={
              active.length > 0
                ? "Clear a filter or widen the range — a session only appears once a student has actually started one."
                : "A row appears here the first time a student opens the classroom and starts talking."
            }
          />

          <Pagination
            offset={offset}
            limit={PAGE_SIZE}
            total={data?.total ?? 0}
            onOffsetChange={setOffset}
          />
        </>
      )}
    </div>
  );
}
