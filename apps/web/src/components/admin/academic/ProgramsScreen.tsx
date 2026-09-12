"use client";

import { useCallback, useState } from "react";

import { useAdminSession } from "@/components/admin/AdminShell";
import { AdminButton, Pagination, SearchInput } from "@/components/admin/controls";
import { DataTable, type Column } from "@/components/admin/DataTable";
import { Banner, PageHeader, StatusPill, statusTone } from "@/components/admin/primitives";
import { ProgramForm } from "@/components/admin/academic/ProgramForm";
import { Blank, CodeTag, RowLink, useResource } from "@/components/admin/academic/shared";
import { listPrograms } from "@/lib/admin/client";
import type { Program } from "@/lib/admin/types";

const PAGE_SIZE = 25;

/**
 * The entry point to the academic hierarchy.
 *
 * A sub-admin sees only the programs they are scoped to — that filtering is
 * the gateway's (`admin/programs.rs` filters by `Actor::in_scope`), so this
 * screen never tries to reproduce it client-side.
 */
export function ProgramsScreen() {
  const { me } = useAdminSession();
  const [q, setQ] = useState("");
  const [offset, setOffset] = useState(0);
  const [form, setForm] = useState<{ program: Program | null } | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const loader = useCallback(
    () => listPrograms({ q: q || undefined, limit: PAGE_SIZE, offset }),
    [q, offset],
  );
  const { data, loading, error, reload } = useResource(loader, "Could not load programs.");

  // Only a super-admin may create a program: the gateway checks the role
  // directly, because there is no `program_id` to scope a new program against
  // yet (`admin/programs.rs`). Hiding the button beats a guaranteed 403.
  const canCreate = me.role === "super_admin";

  const columns: Column<Program>[] = [
    {
      key: "code",
      header: "Code",
      className: "w-[140px]",
      cell: (row) => <CodeTag>{row.code}</CodeTag>,
    },
    {
      key: "name",
      header: "Name",
      cell: (row) => <RowLink href={`/admin/programs/${row.id}`}>{row.name}</RowLink>,
    },
    {
      key: "description",
      header: "Description",
      className: "max-w-[360px]",
      cell: (row) =>
        row.description ? (
          <span className="line-clamp-2 text-gray-500">{row.description}</span>
        ) : (
          <Blank />
        ),
    },
    {
      key: "status",
      header: "Status",
      className: "w-[120px]",
      cell: (row) => <StatusPill tone={statusTone(row.status)}>{row.status}</StatusPill>,
    },
    {
      key: "actions",
      header: "Actions",
      align: "right",
      className: "w-[190px]",
      // "Edit" alone read as the only thing a row could do, which hid the fact
      // that semesters, courses, blocks and units all live one level in. The
      // name is still the primary link; this is the explicit affordance.
      cell: (row) => (
        <div className="flex justify-end gap-2">
          <AdminButton
            type="button"
            variant="outline-light"
            onClick={() => setForm({ program: row })}
          >
            Edit
          </AdminButton>
          <RowLink href={`/admin/programs/${row.id}`}>Manage</RowLink>
        </div>
      ),
    },
  ];

  function handleSaved(message: string) {
    setForm(null);
    setNotice(message);
    reload();
  }

  return (
    <div className="space-y-6">
      <PageHeader
        title="Programs"
        description="Academic programs — e.g. BA Malayalam. Open one to manage its semesters, courses, blocks, and units."
        action={
          canCreate ? (
            <AdminButton type="button" onClick={() => setForm({ program: null })}>
              New program
            </AdminButton>
          ) : null
        }
      />

      {notice ? (
        <Banner tone="success" onDismiss={() => setNotice(null)}>
          {notice}
        </Banner>
      ) : null}
      {error ? <Banner tone="danger">{error}</Banner> : null}

      <div className="flex flex-wrap items-center gap-3">
        <SearchInput
          value={q}
          label="Search programs"
          placeholder="Search by name or code…"
          onChange={(next) => {
            setQ(next);
            setOffset(0);
          }}
        />
      </div>

      <DataTable
        columns={columns}
        rows={data?.items ?? []}
        rowKey={(row) => row.id}
        loading={loading}
        empty={q ? "No programs match that search." : "No programs yet."}
        emptyHint={
          q
            ? "Try a shorter search term."
            : canCreate
              ? "Create the first one with “New program”."
              : "A super admin creates programs."
        }
      />

      <Pagination
        offset={offset}
        limit={PAGE_SIZE}
        total={data?.total ?? 0}
        onOffsetChange={setOffset}
      />

      {/* Mounted only while open, so the dialog starts from the row it was
          opened on without a re-seeding effect. */}
      {form ? (
        <ProgramForm
          program={form.program}
          onClose={() => setForm(null)}
          onSaved={handleSaved}
        />
      ) : null}
    </div>
  );
}
