"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import { useAdminSession } from "@/components/admin/AdminShell";
import {
  CatalogueSection,
  IngestionSection,
  StudentsSection,
} from "@/components/admin/overview/CatalogueSections";
import {
  SafetySection,
  SessionsSection,
  WhiteboardSection,
} from "@/components/admin/overview/LiveSections";
import { ACK_VIOLATION_BUCKET, loadAnalytics } from "@/lib/admin/analytics";
import type { AdminAnalytics } from "@/lib/admin/analytics";
import { toBoardEvents, toIncidents } from "@/components/admin/overview/links";
import { OverviewSkeleton } from "@/components/admin/overview/parts";
import { Banner, PageHeader } from "@/components/admin/primitives";
import { ApiError } from "@/lib/api";

/**
 * The console's landing screen: the platform's state at a glance, and the
 * handful of numbers that are tasks rather than statistics.
 *
 * `GET /admin/analytics` is scoped server-side — a sub-admin's figures cover
 * only their programs, so this screen never has to explain a total that
 * includes content they cannot open, and it never filters client-side.
 *
 * An empty platform is the expected first render. Every section below is
 * written for the all-zero case first: honest zeros, and a one-line note on
 * what would populate the section. Nothing here invents or substitutes data.
 */
export function AdminOverview() {
  const { me } = useAdminSession();
  const [data, setData] = useState<AdminAnalytics | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    loadAnalytics()
      .then((next) => {
        if (!cancelled) setData(next);
      })
      .catch((caught: unknown) => {
        if (cancelled) return;
        setError(
          caught instanceof ApiError ? caught.message : "Could not load the dashboard.",
        );
      });

    return () => {
      cancelled = true;
    };
  }, []);

  const firstName = me.full_name?.split(" ")[0];

  return (
    <div className="space-y-10">
      <PageHeader
        title={firstName ? `Welcome back, ${firstName}` : "Overview"}
        description={
          me.role === "sub_admin"
            ? "Everything below is limited to the programs you are scoped to."
            : "Platform-wide totals across every program."
        }
      />

      {error ? (
        <Banner tone="danger">
          {error} The dashboard reads `GET /admin/analytics`; nothing is shown until it answers.
        </Banner>
      ) : null}

      {!error && !data ? <OverviewSkeleton /> : null}

      {data ? (
        <>
          <Alerts data={data} />

          <CatalogueSection catalogue={data.catalogue} topBlocks={data.series.top_blocks} />
          <StudentsSection students={data.students} daily={data.series.students_daily} />
          <IngestionSection
            documents={data.documents}
            byStatus={data.series.documents_by_status}
            daily={data.series.documents_daily}
          />
          <SessionsSection
            sessions={data.sessions}
            daily={data.series.sessions_daily}
            endReasons={data.series.session_end_reasons}
          />
          <WhiteboardSection
            whiteboard={data.whiteboard}
            buckets={data.series.ack_latency_buckets}
          />
          <SafetySection safety={data.safety} daily={data.series.incidents_daily} />
        </>
      ) : null}
    </div>
  );
}

/**
 * The numbers that are a task, not a statistic — surfaced above the fold so an
 * admin does not have to scroll a dashboard to find out something is wrong.
 * Deliberately silent when there is nothing to say.
 */
function Alerts({ data }: { data: AdminAnalytics }) {
  const overCeiling =
    data.series.ack_latency_buckets.find((row) => row.label === ACK_VIOLATION_BUCKET)?.count ?? 0;
  const nn1 = data.whiteboard.ops_unacked + overCeiling;

  return (
    <div className="space-y-3">
      {nn1 > 0 ? (
        <Banner tone="danger">
          Whiteboard-first (NN-1) violated: {nn1.toLocaleString()} board op
          {nn1 === 1 ? "" : "s"} went unacknowledged or past the 400 ms hold ceiling, so audio
          reached a student before the board did.{" "}
          <AlertLink
            href={toBoardEvents({ violations_only: "true" })}
            label={`View the ${nn1.toLocaleString()} violating board op${nn1 === 1 ? "" : "s"}`}
          />
        </Banner>
      ) : null}

      {data.safety.tier1 > 0 ? (
        <Banner tone="danger">
          {data.safety.tier1.toLocaleString()} Tier-1 safety incident
          {data.safety.tier1 === 1 ? "" : "s"} tore a live session down (close 4009).{" "}
          <AlertLink
            href={toIncidents({ tier: "1" })}
            label={`View the ${data.safety.tier1.toLocaleString()} Tier-1 incident${
              data.safety.tier1 === 1 ? "" : "s"
            }`}
          />
        </Banner>
      ) : null}

      {data.documents.failed > 0 ? (
        <Banner tone="danger">
          {data.documents.failed.toLocaleString()} document
          {data.documents.failed === 1 ? "" : "s"} failed ingestion and are not retrievable.
        </Banner>
      ) : null}

      {/* Low OCR confidence holds a textbook out of the syllabus until a
          sub-admin approves it (`.claude/rules/rag-pipeline.md`). */}
      {data.documents.pending_review > 0 ? (
        <Banner tone="info">
          {data.documents.pending_review.toLocaleString()} document
          {data.documents.pending_review === 1 ? "" : "s"} need review before going live.
        </Banner>
      ) : null}
    </div>
  );
}

/**
 * The "so open them" half of an alert. An alert that states a violation but
 * cannot take you to the rows behind it leaves the admin to go hunting.
 */
function AlertLink({ href, label }: { href: string; label: string }) {
  return (
    <Link
      href={href}
      aria-label={label}
      className="font-medium underline underline-offset-2 hover:no-underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-400 focus-visible:ring-offset-2 focus-visible:ring-offset-lavender-50"
    >
      {label}
    </Link>
  );
}
