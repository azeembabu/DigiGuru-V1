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
  RowLink,
  formatDateTime,
  shortId,
  useUrlFilters,
} from "@/components/admin/drilldown/shared";
import { listSafetyIncidents } from "@/lib/admin/client";
import type { IncidentKind, SafetyIncident } from "@/lib/admin/types";

const TIERS = [
  { value: "0", label: "Tier 0 — upstream muted" },
  { value: "1", label: "Tier 1 — socket torn down" },
  { value: "2", label: "Tier 2 — output dropped" },
];

const KINDS: { value: IncidentKind; label: string }[] = [
  { value: "jailbreak", label: "Jailbreak attempt" },
  { value: "toxicity", label: "Toxicity" },
  { value: "out_of_scope", label: "Out of syllabus" },
];

/** What each tier actually did, spelled out — the number alone means nothing. */
const TIER_EFFECT: Record<string, string> = {
  "0": "Pre-LLM tap: the upstream was muted mid-utterance, so no further audio reached Gemini.",
  "1": "Live transcription hit: the socket was torn down within 300 ms (close code 4009) and the session moved to JAILBREAK_HALT.",
  "2": "Output guard: the model's response was dropped before TTS release rather than spoken.",
};

function headingFor(tier: string, kind: string): { title: string; description: string } {
  if (tier) {
    return {
      title: `Tier-${tier} safety incidents`,
      description: TIER_EFFECT[tier] ?? "Guardrail incidents at this tier.",
    };
  }
  const kindLabel = KINDS.find((item) => item.value === kind);
  if (kindLabel) {
    return {
      title: `${kindLabel.label} incidents`,
      description:
        "Guardrail hits of this kind, newest first. Every incident is persisted, whichever tier caught it.",
    };
  }
  return {
    title: "Safety incidents",
    description:
      "Every NN-5 guardrail hit, newest first. Tier 0 muted the upstream, Tier 1 tore the socket down, Tier 2 dropped the model's output before it was spoken.",
  };
}

function TierPill({ tier }: { tier: number }) {
  // Tier 1 is the only one that ended the student's session, so it is the only
  // one rendered as danger — colouring all three alike would hide that.
  const tone = tier === 1 ? "danger" : tier === 2 ? "warning" : "neutral";
  const label = tier === 1 ? "Tier 1 — session ended" : `Tier ${tier}`;
  return <StatusPill tone={tone}>{label}</StatusPill>;
}

/**
 * The `safety_incidents` drill-down.
 *
 * `excerpt` is rendered exactly as stored. It was redacted at write time
 * (`.claude/rules/security.md`); re-processing it here would either destroy
 * evidence an admin needs or imply a guarantee this screen cannot make.
 */
export function SafetyIncidentsScreen() {
  const { get, offset, set, setOffset, clearAll } = useUrlFilters();
  const tier = get("tier");
  const kind = get("kind");
  const studentId = get("student_id");
  const sessionId = get("session_id");
  const from = get("from");
  const to = get("to");

  const loader = useCallback(
    () =>
      listSafetyIncidents({
        tier: tier || undefined,
        kind: (kind || undefined) as IncidentKind | undefined,
        student_id: studentId || undefined,
        session_id: sessionId || undefined,
        from: from || undefined,
        to: to || undefined,
        limit: PAGE_SIZE,
        offset,
      }),
    [tier, kind, studentId, sessionId, from, to, offset],
  );
  const { data, loading, error } = useResource(loader, "Could not load safety incidents.");

  const heading = headingFor(tier, kind);
  const active: string[] = [];
  if (tier) active.push(TIERS.find((item) => item.value === tier)?.label ?? `Tier ${tier}`);
  if (kind) active.push(KINDS.find((item) => item.value === kind)?.label ?? kind);
  if (studentId) active.push("one student");
  if (sessionId) active.push(`session ${shortId(sessionId)}`);
  if (from || to) active.push("a date range");

  const columns: Column<SafetyIncident>[] = [
    {
      key: "when",
      header: "When",
      cell: (row) => (
        <span className="whitespace-nowrap text-gray-500">{formatDateTime(row.created_at)}</span>
      ),
    },
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
      key: "kind",
      header: "Kind",
      className: "w-[140px]",
      cell: (row) => (
        <span className="whitespace-nowrap">
          {KINDS.find((item) => item.value === row.kind)?.label ?? row.kind}
        </span>
      ),
    },
    {
      key: "tier",
      header: "Tier",
      className: "w-[180px]",
      cell: (row) => <TierPill tier={row.tier} />,
    },
    {
      key: "excerpt",
      header: "Excerpt (redacted at write time)",
      className: "max-w-[360px]",
      cell: (row) => (
        <span className="line-clamp-3 whitespace-pre-wrap break-words text-gray-500">
          {row.excerpt}
        </span>
      ),
    },
    {
      key: "session",
      header: "Session",
      align: "right",
      className: "w-[130px]",
      cell: (row) =>
        row.session_id === null ? (
          <Blank />
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
          id="incident-tier"
          label="Guardrail tier"
          value={tier}
          anyLabel="Any tier"
          options={TIERS}
          onChange={(next) => set({ tier: next })}
        />
        <FilterSelect
          id="incident-kind"
          label="Incident kind"
          value={kind}
          anyLabel="Any kind"
          options={KINDS}
          onChange={(next) => set({ kind: next })}
        />
      </FilterBar>

      <ActiveFilters labels={active} onClear={clearAll} />

      {error ? (
        <Banner tone="danger">{error}</Banner>
      ) : (
        <>
          <DataTable
            columns={columns}
            rows={data?.items ?? []}
            rowKey={(row) => row.id}
            loading={loading}
            empty="No safety incidents match these filters."
            emptyHint={
              active.length > 0
                ? "Clear a filter to see incidents at every tier and kind."
                : "Nothing has tripped a guardrail. A row is written the moment one does, at any tier."
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
