"use client";

import { useCallback, useState } from "react";

import { useAdminSession } from "@/components/admin/AdminShell";
import { AdminButton, Pagination, SearchInput } from "@/components/admin/controls";
import { DataTable, type Column } from "@/components/admin/DataTable";
import { Banner, PageHeader, StatusPill, statusTone } from "@/components/admin/primitives";
import { LscForm } from "@/components/admin/academic/LscForm";
import { Blank, CodeTag, useResource } from "@/components/admin/academic/shared";
import { listLscs } from "@/lib/admin/client";
import type { Lsc } from "@/lib/admin/types";

const PAGE_SIZE = 25;

/**
 * Learner support centres.
 *
 * An LSC has no `program_id`, so it sits outside `sub_admin_scopes` entirely:
 * every admin may read the list, and only a super admin may change it
 * (`apps/gateway/src/admin/lscs.rs`). A sub-admin therefore gets the list with
 * no write controls and a line saying who does own them, rather than buttons
 * that would always come back 403.
 */
export function LscsScreen() {
  const { me } = useAdminSession();
  const [q, setQ] = useState("");
  const [offset, setOffset] = useState(0);
  const [form, setForm] = useState<{ lsc: Lsc | null } | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const loader = useCallback(
    () => listLscs({ q: q || undefined, limit: PAGE_SIZE, offset }),
    [q, offset],
  );
  const { data, loading, error, reload } = useResource(loader, "Could not load the centres.");

  const canWrite = me.role === "super_admin";

  const columns: Column<Lsc>[] = [
    {
      key: "code",
      header: "Code",
      className: "w-[160px]",
      cell: (row) => <CodeTag>{row.code}</CodeTag>,
    },
    { key: "name", header: "Name", cell: (row) => <span className="font-medium">{row.name}</span> },
    {
      key: "location",
      header: "Location",
      className: "max-w-[280px]",
      cell: (row) =>
        row.location ? <span className="text-gray-500">{row.location}</span> : <Blank />,
    },
    {
      key: "status",
      header: "Status",
      className: "w-[120px]",
      cell: (row) => <StatusPill tone={statusTone(row.status)}>{row.status}</StatusPill>,
    },
    ...(canWrite
      ? [
          {
            key: "actions",
            header: "Actions",
            align: "right" as const,
            className: "w-[100px]",
            cell: (row: Lsc) => (
              <AdminButton
                type="button"
                variant="outline-light"
                onClick={() => setForm({ lsc: row })}
              >
                Edit
              </AdminButton>
            ),
          },
        ]
      : []),
  ];

  function handleSaved(message: string) {
    setForm(null);
    setNotice(message);
    reload();
  }

  return (
    <div className="space-y-6">
      <PageHeader
        title="Learner support centres"
        description="Centres students are attached to at signup."
        action={
          canWrite ? (
            <AdminButton type="button" onClick={() => setForm({ lsc: null })}>
              New centre
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

      {canWrite ? null : (
        <Banner tone="info">
          Centres are not part of a program scope, so adding or changing one is handled by a super
          admin. This list is read-only for you.
        </Banner>
      )}

      <div className="flex flex-wrap items-center gap-3">
        <SearchInput
          value={q}
          label="Search centres"
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
        empty={q ? "No centres match that search." : "No centres yet."}
        emptyHint={q ? "Try a shorter search term." : undefined}
      />

      <Pagination
        offset={offset}
        limit={PAGE_SIZE}
        total={data?.total ?? 0}
        onOffsetChange={setOffset}
      />

      {form && canWrite ? (
        <LscForm lsc={form.lsc} onClose={() => setForm(null)} onSaved={handleSaved} />
      ) : null}
    </div>
  );
}
