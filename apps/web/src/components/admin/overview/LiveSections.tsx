// Live tutoring sessions, whiteboard sync (NN-1), and safety (NN-5).
//
// These three read the runtime rather than the catalogue, and two of them are
// non-negotiable-bearing, so the tone is not decorative:
//   * any `ops_unacked`, or any count in the `>400ms` ACK bucket, is a
//     whiteboard-first violation — audio that reached the student ahead of the
//     board (`.claude/rules/whiteboard-sync.md`), which CI treats as a release
//     blocker. It is rendered `bad`, never as a neutral latency statistic.
//   * any Tier-1 safety incident is a socket teardown that already happened
//     (`.claude/rules/security.md`).

"use client";

import { BarChart, DonutChart, LineChart, ProgressBar, StatTile } from "@/components/charts";
import type {
  AnalyticsSeries,
  SafetyMetrics,
  SessionMetrics,
  WhiteboardMetrics,
} from "@/lib/admin/analytics";
import { ACK_VIOLATION_BUCKET } from "@/lib/admin/analytics";

import {
  ACCENT,
  badIfAny,
  dayLabel,
  duration,
  goodIfAny,
  humanLabel,
  millis,
  n,
  ratio,
  shareOf,
  trendOf,
  warnIfAny,
} from "./format";
import {
  countWords,
  since,
  toBoardEvents,
  toIncidents,
  toSessions,
  type EndReason,
} from "./links";
import { ChartCard, ChartLinks, MetricList, Section, SplitGrid, TileGrid } from "./parts";

const NO_SESSIONS_HINT = "No sessions yet — figures appear once students start a class.";

export function SessionsSection({
  sessions,
  daily,
  endReasons,
}: {
  sessions: SessionMetrics;
  daily: AnalyticsSeries["sessions_daily"];
  endReasons: AnalyticsSeries["session_end_reasons"];
}) {
  const trend = trendOf(daily, (row) => row.count);
  const empty = sessions.total === 0;

  return (
    <Section
      id="sessions-heading"
      title="Live tutoring sessions"
      description="Voice time is server-authoritative and counted only while voice is active (NN-3)."
    >
      <TileGrid>
        <StatTile
          label="Sessions"
          value={n(sessions.total)}
          trend={trend}
          href={toSessions()}
          linkLabel={`View all ${countWords(sessions.total)}tutoring sessions`}
        />
        <StatTile
          label="In progress"
          value={n(sessions.in_progress)}
          hint="Sockets open right now."
          tone={goodIfAny(sessions.in_progress)}
          href={toSessions({ status: "in_progress" })}
          linkLabel={`View ${countWords(sessions.in_progress)}sessions in progress`}
        />
        <StatTile
          label="Voice time taught"
          value={duration(sessions.active_voice_ms_total)}
          hint={`Mean ${duration(sessions.active_voice_ms_avg)} per session`}
        />
        <StatTile
          label="Hit the 20-minute cap"
          value={n(sessions.ended_quota)}
          hint="Ended by the hard NN-3 quota, not by the student."
          tone={warnIfAny(sessions.ended_quota)}
          href={toSessions({ end_reason: "quota" })}
          linkLabel={`View ${countWords(sessions.ended_quota)}sessions that hit the 20-minute cap`}
        />
      </TileGrid>

      <SplitGrid>
        <ChartCard
          title="Sessions — last 30 days"
          hint="Session count, with voice minutes as the secondary series."
          empty={empty}
          emptyHint={NO_SESSIONS_HINT}
        >
          <LineChart
            data={daily.map((row) => ({
              label: dayLabel(row.day),
              value: row.count,
              secondary: Math.round(row.voice_ms / 60000),
            }))}
            label="Sessions"
            accent={ACCENT.indigo}
            valueFormat={n}
          />
        </ChartCard>

        <ChartCard
          title="How sessions ended"
          hint="Fixed label set — quota, idle, user, jailbreak, error."
          empty={empty}
          emptyHint={NO_SESSIONS_HINT}
        >
          <DonutChart
            data={endReasons.map((row) => ({ label: humanLabel(row.label), value: row.count }))}
            centerLabel={`${n(sessions.total)} sessions`}
            valueFormat={n}
          />
          <ChartLinks
            title="Open the sessions behind each end reason"
            links={endReasons.map((row) => ({
              label: humanLabel(row.label),
              value: n(row.count),
              href: toSessions({ end_reason: row.label as EndReason }),
              linkLabel: `View ${countWords(row.count)}sessions that ended with ${row.label}`,
            }))}
          />
        </ChartCard>
      </SplitGrid>

      <SplitGrid>
        <MetricList
          title="Session outcomes"
          rows={[
            {
              label: "Completed",
              value: n(sessions.completed),
              hint: shareOf(sessions.completed, sessions.total),
              tone: goodIfAny(sessions.completed),
              href: toSessions({ status: "completed" }),
              linkLabel: `View ${countWords(sessions.completed)}completed sessions`,
            },
            {
              label: "Abandoned",
              value: n(sessions.abandoned),
              tone: warnIfAny(sessions.abandoned),
              href: toSessions({ status: "abandoned" }),
              linkLabel: `View ${countWords(sessions.abandoned)}abandoned sessions`,
            },
            {
              label: "Started in the last 7 days",
              value: n(sessions.last_7d),
              href: toSessions({ from: since(7) }),
              linkLabel: `View ${countWords(sessions.last_7d)}sessions started in the last 7 days`,
            },
            { label: "Total voice time", value: duration(sessions.active_voice_ms_total) },
          ]}
        />
        <MetricList
          title="End reasons"
          hint="A jailbreak end is a guardrail teardown, not a student choice."
          rows={[
            {
              label: "Quota reached (close 4003)",
              value: n(sessions.ended_quota),
              href: toSessions({ end_reason: "quota" }),
              linkLabel: `View ${countWords(sessions.ended_quota)}sessions ended by the quota`,
            },
            {
              label: "Idle timeout (close 4008)",
              value: n(sessions.ended_idle),
              href: toSessions({ end_reason: "idle" }),
              linkLabel: `View ${countWords(sessions.ended_idle)}sessions ended by the idle timeout`,
            },
            {
              label: "Ended by the student",
              value: n(sessions.ended_user),
              href: toSessions({ end_reason: "user" }),
              linkLabel: `View ${countWords(sessions.ended_user)}sessions ended by the student`,
            },
            {
              label: "Safety termination (close 4009)",
              value: n(sessions.ended_jailbreak),
              tone: badIfAny(sessions.ended_jailbreak),
              href: toSessions({ end_reason: "jailbreak" }),
              linkLabel: `View ${countWords(sessions.ended_jailbreak)}sessions torn down by a guardrail`,
            },
            {
              label: "Upstream or gateway error",
              value: n(sessions.ended_error),
              tone: badIfAny(sessions.ended_error),
              href: toSessions({ end_reason: "error" }),
              linkLabel: `View ${countWords(sessions.ended_error)}sessions ended by an error`,
            },
          ]}
        />
      </SplitGrid>
    </Section>
  );
}

export function WhiteboardSection({
  whiteboard,
  buckets,
}: {
  whiteboard: WhiteboardMetrics;
  buckets: AnalyticsSeries["ack_latency_buckets"];
}) {
  const empty = whiteboard.ops_total === 0;
  const overCeiling = buckets.find((row) => row.label === ACK_VIOLATION_BUCKET)?.count ?? 0;
  // Either signal means audio was released without the board landing first.
  const violating = whiteboard.ops_unacked > 0 || overCeiling > 0;
  const ackedShare = empty ? 0 : whiteboard.ops_acked / whiteboard.ops_total;

  return (
    <Section
      id="whiteboard-heading"
      title="Whiteboard sync (NN-1)"
      description="Audio for a turn is never released before the client ACKs that turn's board ops, or the 400 ms hold ceiling expires."
    >
      <TileGrid>
        <StatTile
          label="Board ops"
          value={n(whiteboard.ops_total)}
          href={toBoardEvents()}
          linkLabel={`View all ${countWords(whiteboard.ops_total)}board ops`}
        />
        <StatTile
          label="Acknowledged"
          value={n(whiteboard.ops_acked)}
          hint={shareOf(whiteboard.ops_acked, whiteboard.ops_total)}
          tone={goodIfAny(whiteboard.ops_acked)}
        />
        <StatTile
          label="Never acknowledged"
          value={n(whiteboard.ops_unacked)}
          hint={
            whiteboard.ops_unacked > 0
              ? "NN-1 violation — audio was released on the hold timer."
              : "No turn outran its board."
          }
          tone={badIfAny(whiteboard.ops_unacked)}
          href={toBoardEvents({ violations_only: "true" })}
          linkLabel={`View ${countWords(whiteboard.ops_unacked)}board ops that violated whiteboard-first`}
        />
        <StatTile
          label="Violation rate"
          value={ratio(whiteboard.violation_rate, 2)}
          hint="Must be 0% — the same quantity CI blocks a release on."
          tone={empty ? "default" : violating ? "bad" : "good"}
        />
      </TileGrid>

      <SplitGrid>
        <ChartCard
          title="ACK latency"
          hint={`Anything in ${ACK_VIOLATION_BUCKET} is past the HOLD_MAX ceiling and counts as a violation.`}
          empty={empty}
          emptyHint="No board ops recorded yet — the board is measured during a live session."
        >
          <BarChart
            data={buckets.map((row) => ({ label: row.label, value: row.count }))}
            accent={ACCENT.indigo}
            highlightLabels={[ACK_VIOLATION_BUCKET]}
            valueFormat={n}
          />
          <ChartLinks
            title="Open the board ops in each latency bucket"
            links={buckets.map((row) => ({
              label: row.label,
              value: n(row.count),
              href: toBoardEvents({ bucket: row.label }),
              linkLabel: `View ${countWords(row.count)}board ops acknowledged in ${row.label}`,
            }))}
          />
        </ChartCard>

        <div className="min-w-0 space-y-4">
          <MetricList
            title="Round-trip"
            hint="Board-op emission to client ACK."
            rows={[
              { label: "Median (p50)", value: millis(whiteboard.ack_p50_ms) },
              {
                label: "p95",
                value: millis(whiteboard.ack_p95_ms),
                hint: "Budgeted at 250 ms typical, 400 ms ceiling.",
                tone:
                  whiteboard.ack_p95_ms === null
                    ? "default"
                    : whiteboard.ack_p95_ms > 400
                      ? "bad"
                      : whiteboard.ack_p95_ms > 250
                        ? "warn"
                        : "good",
              },
              {
                label: `Ops over ${ACK_VIOLATION_BUCKET}`,
                value: n(overCeiling),
                tone: badIfAny(overCeiling),
                href: toBoardEvents({ bucket: ACK_VIOLATION_BUCKET }),
                linkLabel: `View ${countWords(overCeiling)}board ops past the 400 ms hold ceiling`,
              },
            ]}
          />
          <div className="px-1">
            <ProgressBar
              value={ackedShare}
              label="Ops acknowledged before the hold expired"
              tone={empty ? "default" : violating ? "bad" : "good"}
            />
          </div>
        </div>
      </SplitGrid>
    </Section>
  );
}

export function SafetySection({
  safety,
  daily,
}: {
  safety: SafetyMetrics;
  daily: AnalyticsSeries["incidents_daily"];
}) {
  const trend = trendOf(daily, (row) => row.count);
  const empty = safety.total === 0;

  return (
    <Section
      id="safety-heading"
      title="Safety (NN-5)"
      description="Tier 0 mutes the upstream mid-utterance; Tier 1 tears the socket down. Every incident is persisted."
    >
      <TileGrid>
        <StatTile
          label="Incidents"
          value={n(safety.total)}
          trend={trend}
          tone={badIfAny(safety.total)}
          href={toIncidents()}
          linkLabel={`View all ${countWords(safety.total)}safety incidents`}
        />
        <StatTile
          label="Tier 0 — muted"
          value={n(safety.tier0)}
          hint="Caught pre-LLM; no audio reached Gemini."
          tone={warnIfAny(safety.tier0)}
          href={toIncidents({ tier: "0" })}
          linkLabel={`View ${countWords(safety.tier0)}Tier-0 safety incidents`}
        />
        <StatTile
          label="Tier 1 — socket torn down"
          value={n(safety.tier1)}
          hint="Each one ended a student's session within 300 ms."
          tone={badIfAny(safety.tier1)}
          href={toIncidents({ tier: "1" })}
          linkLabel={`View ${countWords(safety.tier1)}Tier-1 safety incidents`}
        />
        <StatTile
          label="Last 7 days"
          value={n(safety.last_7d)}
          hint="Recent pressure, not lifetime total."
          tone={badIfAny(safety.last_7d)}
          href={toIncidents({ from: since(7) })}
          linkLabel={`View ${countWords(safety.last_7d)}safety incidents from the last 7 days`}
        />
      </TileGrid>

      <SplitGrid>
        <ChartCard
          title="Incidents — last 30 days"
          empty={empty}
          emptyHint="No safety incidents recorded. A flat zero here is the expected state."
        >
          <LineChart
            data={daily.map((row) => ({ label: dayLabel(row.day), value: row.count }))}
            label="Incidents"
            accent={ACCENT.danger}
            valueFormat={n}
          />
        </ChartCard>

        <MetricList
          title="What was caught"
          rows={[
            {
              label: "Tier 2 — output dropped",
              value: n(safety.tier2),
              hint: "Uncited or prompt-leaking output, blocked before TTS.",
              tone: warnIfAny(safety.tier2),
              href: toIncidents({ tier: "2" }),
              linkLabel: `View ${countWords(safety.tier2)}Tier-2 safety incidents`,
            },
            {
              label: "Jailbreak attempts",
              value: n(safety.jailbreak),
              tone: badIfAny(safety.jailbreak),
              href: toIncidents({ kind: "jailbreak" }),
              linkLabel: `View ${countWords(safety.jailbreak)}jailbreak incidents`,
            },
            {
              label: "Toxicity",
              value: n(safety.toxicity),
              tone: badIfAny(safety.toxicity),
              href: toIncidents({ kind: "toxicity" }),
              linkLabel: `View ${countWords(safety.toxicity)}toxicity incidents`,
            },
            {
              label: "Out of syllabus",
              value: n(safety.out_of_scope),
              hint: "Abstained on, per NN-4 — expected traffic, not an attack.",
              href: toIncidents({ kind: "out_of_scope" }),
              linkLabel: `View ${countWords(safety.out_of_scope)}out-of-syllabus incidents`,
            },
          ]}
        />
      </SplitGrid>
    </Section>
  );
}
