"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import { useAdminSession } from "@/components/admin/AdminShell";
import { Banner, PageHeader, StatusPill, panelClass } from "@/components/admin/primitives";
import { loadStats } from "@/lib/admin/client";
import type { AdminStats } from "@/lib/admin/types";
import { ApiError } from "@/lib/api";

/**
 * The console's landing screen: counts, and the one thing that actually needs
 * an admin's attention.
 *
 * `GET /admin/stats` is scoped server-side — a sub-admin's numbers cover only
 * their programs, so this screen never has to explain a total that includes
 * content they cannot open.
 */
export function AdminOverview() {
  const { me } = useAdminSession();
  const [stats, setStats] = useState<AdminStats | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    loadStats()
      .then((next) => {
        if (!cancelled) setStats(next);
      })
      .catch((caught: unknown) => {
        if (cancelled) return;
        setError(
          caught instanceof ApiError ? caught.message : "Could not load the summary.",
        );
      });

    return () => {
      cancelled = true;
    };
  }, []);

  const firstName = me.full_name?.split(" ")[0];

  return (
    <div className="space-y-6">
      <PageHeader
        title={firstName ? `Welcome back, ${firstName}` : "Overview"}
        description={
          me.role === "sub_admin"
            ? "Everything below is limited to the programs you are scoped to."
            : "Platform-wide totals across every program."
        }
      />

      {error ? <Banner tone="danger">{error}</Banner> : null}

      {/* A pending-review document is the one number that is a task, not a
          statistic — low OCR confidence holds a textbook out of the syllabus
          until a sub-admin approves it (`.claude/rules/rag-pipeline.md`). */}
      {stats && stats.documents.pending_review > 0 ? (
        <Banner tone="info">
          {stats.documents.pending_review} document
          {stats.documents.pending_review === 1 ? "" : "s"} need review before going live.
        </Banner>
      ) : null}

      <section aria-label="Totals" className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
        <StatCard label="Students" value={stats?.students} href="/admin/students" />
        <StatCard label="Programs" value={stats?.programs} href="/admin/programs" />
        <StatCard label="Learner support centres" value={stats?.lscs} href="/admin/lscs" />
        <StatCard label="Semesters" value={stats?.semesters} />
        <StatCard label="Courses" value={stats?.courses} />
        <StatCard label="Blocks" value={stats?.blocks} />
      </section>

      <section aria-labelledby="ingestion-heading" className={`${panelClass} p-5`}>
        <h2 id="ingestion-heading" className="font-display text-base font-semibold text-gray-900">
          Textbook ingestion
        </h2>
        <p className="mt-1 text-sm text-gray-500">
          A block cannot be taught from a document that has not finished embedding.
        </p>

        <dl className="mt-4 flex flex-wrap gap-x-8 gap-y-3">
          <IngestStat label="Uploaded" value={stats?.documents.total} />
          <IngestStat label="Embedded" value={stats?.documents.embedded} tone="success" />
          <IngestStat
            label="Awaiting review"
            value={stats?.documents.pending_review}
            tone="warning"
          />
          <IngestStat label="Failed" value={stats?.documents.failed} tone="danger" />
        </dl>
      </section>
    </div>
  );
}

function StatCard({ label, value, href }: { label: string; value?: number; href?: string }) {
  const body = (
    <>
      <p className="text-sm text-gray-500">{label}</p>
      <p className="mt-1 font-display text-3xl font-semibold text-gray-900 tabular-nums">
        {value === undefined ? (
          <span className="inline-block h-8 w-16 animate-pulse rounded-sm bg-lavender-100 align-middle" />
        ) : (
          value.toLocaleString()
        )}
      </p>
    </>
  );

  if (!href) {
    return <div className={`${panelClass} p-5`}>{body}</div>;
  }

  return (
    <Link
      href={href}
      className={`${panelClass} block p-5 transition-colors hover:border-gray-500/40 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-400 focus-visible:ring-offset-2 focus-visible:ring-offset-lavender-50`}
    >
      {body}
    </Link>
  );
}

function IngestStat({
  label,
  value,
  tone = "neutral",
}: {
  label: string;
  value?: number;
  tone?: "neutral" | "success" | "warning" | "danger";
}) {
  return (
    <div>
      <dt className="text-xs font-medium uppercase tracking-wide text-gray-500">{label}</dt>
      <dd className="mt-1">
        {value === undefined ? (
          <span className="inline-block h-5 w-10 animate-pulse rounded-sm bg-lavender-100" />
        ) : tone === "neutral" ? (
          <span className="font-display text-xl font-semibold tabular-nums">{value}</span>
        ) : (
          <StatusPill tone={tone}>{value}</StatusPill>
        )}
      </dd>
    </div>
  );
}
