"use client";

import { useCallback } from "react";

import { DataTable, type Column } from "@/components/admin/DataTable";
import { Pagination } from "@/components/admin/controls";
import { Banner, PageHeader, StatusPill } from "@/components/admin/primitives";
import { useResource } from "@/components/admin/academic/shared";
import {
  ActiveFilters,
  FilterBar,
  FilterSelect,
  HOLD_MAX_MS,
  PAGE_SIZE,
  RowLink,
  formatDateTime,
  shortId,
  useUrlFilters,
} from "@/components/admin/drilldown/shared";
import { listBoardEvents } from "@/lib/admin/client";
import type { BoardEvent, BoardLatencyBucket } from "@/lib/admin/types";

const BUCKETS: { value: BoardLatencyBucket; label: string }[] = [
  { value: "0-100ms", label: "ACK under 100 ms" },
  { value: "100-250ms", label: "ACK 100–250 ms" },
  { value: "250-400ms", label: "ACK 250–400 ms" },
  { value: ">400ms", label: "ACK over 400 ms" },
];

const VIOLATIONS = [{ value: "true", label: "Violations only" }];

function headingFor(
  sessionId: string,
  violationsOnly: boolean,
  bucket: string,
): { title: string; description: string } {
  if (sessionId && violationsOnly) {
    return {
      title: "Whiteboard violations in one session",
      description:
        "The ops in this session where the board did not land before the audio — in turn order, so the run of turns around each breach reads in sequence.",
    };
  }
  if (sessionId) {
    return {
      title: "Whiteboard transcript for one session",
      description:
        "Every board op the tutor emitted, in turn order, with the time the client took to acknowledge it. An op acknowledged above the 400 ms HOLD_MAX — or never acknowledged — is an NN-1 breach.",
    };
  }
  if (violationsOnly) {
    return {
      title: "Whiteboard-first (NN-1) violations",
      description:
        "Board ops the client never acknowledged, or acknowledged after the 400 ms HOLD_MAX hold expired — so audio for that turn may have reached the student before the visual did. CI treats a non-zero count as a release blocker.",
    };
  }
  if (bucket) {
    const label = BUCKETS.find((item) => item.value === bucket)?.label ?? bucket;
    return {
      title: `Board ops with ${label.replace("ACK ", "an ACK ")}`,
      description: "Ops in one ACK-latency bucket, newest first.",
    };
  }
  return {
    title: "Whiteboard board ops",
    description:
      "Every op emitted to a classroom board, newest first, with its SyncGate ACK latency. Open a single session to read it as a transcript instead.",
  };
}

/** The ACK latency, judged against `HOLD_MAX`, never as a neutral number. */
function AckLatency({ event }: { event: BoardEvent }) {
  if (event.acked_ms === null) {
    return <StatusPill tone="danger">never acked</StatusPill>;
  }
  if (event.is_violation || event.acked_ms > HOLD_MAX_MS) {
    return <StatusPill tone="danger">{event.acked_ms} ms</StatusPill>;
  }
  return <span className="whitespace-nowrap tabular-nums text-gray-900">{event.acked_ms} ms</span>;
}

/**
 * The `board_events` drill-down.
 *
 * With `session_id` the gateway orders by `turn_seq` ascending, so the list is
 * a transcript rather than a reverse feed — the turn number leads the row and
 * the heading says so, because "first row is oldest" reverses between the two
 * modes and an admin must not have to infer which one they are in.
 */
export function BoardEventsScreen() {
  const { get, offset, set, setOffset, clearAll } = useUrlFilters();
  const sessionId = get("session_id");
  const violationsOnly = get("violations_only") === "true";
  const bucket = get("bucket");

  const loader = useCallback(
    () =>
      listBoardEvents({
        session_id: sessionId || undefined,
        violations_only: violationsOnly ? "true" : undefined,
        bucket: (bucket || undefined) as BoardLatencyBucket | undefined,
        limit: PAGE_SIZE,
        offset,
      }),
    [sessionId, violationsOnly, bucket, offset],
  );
  const { data, loading, error } = useResource(loader, "Could not load board events.");

  const heading = headingFor(sessionId, violationsOnly, bucket);
  const active: string[] = [];
  if (sessionId) active.push(`session ${shortId(sessionId)}`);
  if (violationsOnly) active.push("violations only");
  if (bucket) active.push(BUCKETS.find((item) => item.value === bucket)?.label ?? bucket);

  const columns: Column<BoardEvent>[] = [
    {
      key: "turn",
      header: "Turn",
      className: "w-[90px]",
      cell: (row) => <span className="font-mono text-xs text-gray-900">#{row.turn_seq}</span>,
    },
    {
      key: "kind",
      header: "Op",
      className: "w-[120px]",
      cell: (row) => (
        <span className="inline-flex items-center rounded-sm bg-lavender-100 px-1.5 py-0.5 font-mono text-xs text-gray-900">
          {row.op_kind}
        </span>
      ),
    },
    {
      key: "emitted",
      header: "Emitted",
      cell: (row) => (
        <span className="whitespace-nowrap text-gray-500">{formatDateTime(row.emitted_at)}</span>
      ),
    },
    {
      key: "ack",
      header: "ACK latency",
      align: "right",
      className: "w-[150px]",
      cell: (row) => <AckLatency event={row} />,
    },
    {
      key: "verdict",
      header: "NN-1",
      className: "w-[150px]",
      cell: (row) =>
        row.is_violation ? (
          <StatusPill tone="danger">violation</StatusPill>
        ) : (
          <StatusPill tone="success">in time</StatusPill>
        ),
    },
    {
      key: "session",
      header: "Session",
      align: "right",
      className: "w-[150px]",
      cell: (row) =>
        sessionId ? (
          // Already scoped to one session — repeating its id per row is noise.
          <span className="text-gray-500">—</span>
        ) : (
          <RowLink href={`/admin/board-events?session_id=${row.session_id}`}>
            {shortId(row.session_id)}
          </RowLink>
        ),
    },
  ];

  return (
    <div className="space-y-6">
      <PageHeader title={heading.title} description={heading.description} />

      <FilterBar>
        <FilterSelect
          id="board-violations"
          label="Violations"
          value={violationsOnly ? "true" : ""}
          anyLabel="All board ops"
          options={VIOLATIONS}
          onChange={(next) => set({ violations_only: next })}
        />
        <FilterSelect
          id="board-bucket"
          label="ACK latency bucket"
          value={bucket}
          anyLabel="Any ACK latency"
          options={BUCKETS}
          onChange={(next) => set({ bucket: next })}
        />
        {sessionId ? (
          <RowLink href="/admin/sessions">Back to sessions</RowLink>
        ) : null}
      </FilterBar>

      <ActiveFilters labels={active} onClear={clearAll} />

      {error ? (
        <Banner tone="danger">{error}</Banner>
      ) : (
        <>
          <DataTable
            columns={columns}
            rows={data?.items ?? []}
            rowKey={(row) => String(row.id)}
            loading={loading}
            empty={
              violationsOnly
                ? "No whiteboard violations here — which is the result you want."
                : "No board ops match these filters."
            }
            emptyHint={
              violationsOnly
                ? "Every board op in scope was acknowledged inside the 400 ms hold, so audio never outran the board."
                : sessionId
                  ? "This session emitted no board ops — check that it got past session_ready."
                  : "Ops are recorded as the tutor draws; they appear once students hold live sessions."
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
