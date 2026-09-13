"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import { useRouter, useSearchParams } from "next/navigation";

import { useAdminSession } from "@/components/admin/AdminShell";
import { AdminSelect, Pagination, SearchInput } from "@/components/admin/controls";
import { DataTable, type Column } from "@/components/admin/DataTable";
import {
  Banner,
  PageHeader,
  StatusPill,
  panelClass,
  statusTone,
} from "@/components/admin/primitives";
import {
  formatDate,
  nameOf,
  useCatalogue,
} from "@/components/admin/people/lookups";
import { listStudents } from "@/lib/admin/client";
import type { Student } from "@/lib/admin/types";
import { ApiError } from "@/lib/api";

const PAGE_SIZE = 25;

// The gateway validates `status` against the `users.status` enum and answers a
// 400 for anything else, so an unknown value in the URL is dropped rather than
// forwarded — a hand-edited link should show the unfiltered list, not an error.
const ALLOWED_STATUSES = ["active", "inactive", "suspended"];

/**
 * The roster.
 *
 * Search and the status filter are server-side (`q`, `status` on
 * `GET /admin/students`); paging is `limit`/`offset` against `X-Total-Count`.
 * Nothing is filtered again in the browser — a sub-admin's rows are already
 * restricted to their `sub_admin_scopes` programs at the SQL layer
 * (`apps/gateway/src/admin/students.rs`), and re-filtering here would only
 * make the page count lie.
 */
export function StudentsList() {
  const { me } = useAdminSession();
  const catalogue = useCatalogue();

  // Seeded from the URL so a dashboard metric can deep-link into a filtered
  // view ("Suspended accounts" -> ?status=suspended) and so the filtered list
  // stays shareable. Only the initial value is read: the controls below own
  // the state from then on, and push the change back to the URL.
  const searchParams = useSearchParams();
  const router = useRouter();
  const initialStatus = ALLOWED_STATUSES.includes(searchParams.get("status") ?? "")
    ? (searchParams.get("status") as string)
    : "";

  const [q, setQ] = useState(() => searchParams.get("q") ?? "");
  const [status, setStatus] = useState(initialStatus);
  const [offset, setOffset] = useState(0);
  const [rows, setRows] = useState<Student[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // A changed filter invalidates the current page, so it resets to the first
  // one here at the source of the change rather than in an effect — an effect
  // would render the stale offset once before correcting it.
  // Mirror the filters into the URL so the address bar always describes what
  // is on screen — `replace`, not `push`, so typing a search term does not
  // bury the previous page under a stack of history entries.
  function syncUrl(nextQ: string, nextStatus: string) {
    const params = new URLSearchParams();
    if (nextQ) params.set("q", nextQ);
    if (nextStatus) params.set("status", nextStatus);
    const query = params.toString();
    router.replace(query ? `/admin/students?${query}` : "/admin/students", { scroll: false });
  }

  function onSearch(next: string) {
    setQ(next);
    setOffset(0);
    syncUrl(next, status);
  }

  function onStatus(next: string) {
    setStatus(next);
    setOffset(0);
    syncUrl(q, next);
  }

  useEffect(() => {
    let cancelled = false;

    // The fetch runs inside an async body rather than the effect body itself:
    // React 19's lint rule treats a synchronous setState in an effect as a
    // cascading render, and the spinner belongs to the request, not the render.
    void (async () => {
      setLoading(true);
      try {
        const page = await listStudents({
          q: q || undefined,
          status: status || undefined,
          limit: PAGE_SIZE,
          offset,
        });
        if (cancelled) return;
        setRows(page.items);
        setTotal(page.total);
        setError(null);
      } catch (caught: unknown) {
        if (cancelled) return;
        setRows([]);
        setTotal(0);
        setError(
          caught instanceof ApiError ? caught.message : "Could not load the student list.",
        );
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [q, status, offset]);

  const columns: Column<Student>[] = [
    {
      key: "name",
      header: "Student",
      cell: (row) => (
        <div className="min-w-[10rem]">
          <p className="font-medium text-gray-900">{row.full_name}</p>
          <p className="mt-0.5 font-mono text-xs text-gray-500">{row.phone_number}</p>
        </div>
      ),
    },
    {
      key: "roll",
      header: "Roll no.",
      cell: (row) => <span className="whitespace-nowrap font-mono text-xs">{row.roll_number}</span>,
    },
    {
      key: "program",
      header: "Program",
      cell: (row) => nameOf(catalogue.programNames, row.program_id),
    },
    {
      key: "semester",
      header: "Semester",
      // Resolved server-side on `StudentResponse`, so no per-program fan-out.
      cell: (row) => `${row.semester_number} · ${row.semester_name}`,
    },
    {
      key: "lsc",
      header: "LSC",
      cell: (row) => nameOf(catalogue.lscNames, row.lsc_id),
    },
    {
      key: "status",
      header: "Status",
      // The linked account's status — the same field the filter above matches,
      // so a filtered list can be read back against the column it filtered on.
      cell: (row) => <StatusPill tone={statusTone(row.status)}>{row.status}</StatusPill>,
    },
    {
      key: "block",
      header: "Current block",
      cell: (row) =>
        row.current_block_id ? (
          <StatusPill tone="success">Placed</StatusPill>
        ) : (
          <StatusPill tone="warning">Not placed</StatusPill>
        ),
    },
    {
      key: "first_login",
      header: "First login",
      cell: (row) =>
        row.is_first_login ? (
          <StatusPill tone="info">Pending</StatusPill>
        ) : (
          <span className="text-gray-500">Done</span>
        ),
    },
    {
      key: "created",
      header: "Joined",
      cell: (row) => <span className="whitespace-nowrap tabular-nums">{formatDate(row.created_at)}</span>,
    },
    {
      key: "actions",
      header: "",
      align: "right",
      cell: (row) => (
        // The id, never the name — a student's name must not reach a URL
        // (`.claude/rules/security.md`: log identifiers, never PII).
        <Link
          href={`/admin/students/${row.id}`}
          className="rounded-sm font-medium text-indigo-500 underline-offset-2 hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-400 focus-visible:ring-offset-2 focus-visible:ring-offset-white"
        >
          Open<span className="sr-only"> {row.roll_number}</span>
        </Link>
      ),
    },
  ];

  const scopedAndEmpty =
    !loading && rows.length === 0 && !q && !status && me.role === "sub_admin";

  return (
    <div className="space-y-6">
      <PageHeader
        title="Students"
        description={
          me.role === "sub_admin"
            ? "Only students in the programs you are scoped to."
            : "Every registered student across all programs."
        }
      />

      {error ? <Banner tone="danger">{error}</Banner> : null}

      {/* An empty roster reads as a bug unless the scope that produced it is
          visible — a sub-admin with no scopes can never see a single row. */}
      {scopedAndEmpty ? (
        <Banner tone="info">
          {me.scopes.length === 0
            ? "You are not scoped to any program yet, so no students are visible. A super admin assigns scopes."
            : `You are scoped to ${me.scopes.map((scope) => scope.name).join(", ")}. Students outside those programs are not listed.`}
        </Banner>
      ) : null}

      <div className={`${panelClass} flex flex-wrap items-end gap-3 p-4`}>
        <SearchInput
          value={q}
          onChange={onSearch}
          label="Search students"
          placeholder="Name, roll number, or phone…"
        />
        <div className="w-full sm:w-48">
          <label htmlFor="student-status" className="sr-only">
            Filter by status
          </label>
          <AdminSelect
            id="student-status"
            value={status}
            onChange={(event) => onStatus(event.target.value)}
          >
            <option value="">All statuses</option>
            <option value="active">Active</option>
            <option value="inactive">Inactive</option>
            <option value="suspended">Suspended</option>
          </AdminSelect>
        </div>
      </div>

      <DataTable
        columns={columns}
        rows={rows}
        rowKey={(row) => row.id}
        loading={loading || catalogue.loading}
        empty={q || status ? "No students match those filters" : "No students yet"}
        emptyHint={
          q || status
            ? "Try a different search term or clear the status filter."
            : "Students register themselves; they appear here once they sign up."
        }
      />

      <Pagination offset={offset} limit={PAGE_SIZE} total={total} onOffsetChange={setOffset} />
    </div>
  );
}
