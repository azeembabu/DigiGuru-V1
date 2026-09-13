"use client";

import { useCallback, useEffect, useState } from "react";

import { useAdminSession } from "@/components/admin/AdminShell";
import { AdminButton, AdminSelect, Pagination, SearchInput } from "@/components/admin/controls";
import { ConfirmDialog } from "@/components/admin/Modal";
import { DataTable, type Column } from "@/components/admin/DataTable";
import {
  Banner,
  EmptyState,
  PageHeader,
  StatusPill,
  panelClass,
  statusTone,
} from "@/components/admin/primitives";
import { NewAdminModal } from "@/components/admin/people/NewAdminModal";
import { UserScopesModal } from "@/components/admin/people/UserScopesModal";
import { formatDate, useCatalogue } from "@/components/admin/people/lookups";
import { listUsers, setUserStatus } from "@/lib/admin/client";
import type { AdminUser, Role, UserStatus } from "@/lib/admin/types";
import { ApiError } from "@/lib/api";

const PAGE_SIZE = 25;

const ROLE_LABEL: Record<Role, string> = {
  super_admin: "Super admin",
  sub_admin: "Sub-admin",
  student: "Student",
};

const STATUS_LABEL: Record<UserStatus, string> = {
  active: "Active",
  inactive: "Inactive",
  suspended: "Suspended",
};

function isRole(value: string): value is Role {
  return value === "super_admin" || value === "sub_admin" || value === "student";
}

function isStatus(value: string): value is UserStatus {
  return value === "active" || value === "inactive" || value === "suspended";
}

/**
 * Admin accounts. Super-admin only — the gateway refuses every handler behind
 * this screen for anyone else, so a sub-admin is told that plainly instead of
 * being walked into a page of 403s.
 */
export function AdminUsers() {
  const { me } = useAdminSession();

  if (me.role !== "super_admin") {
    return (
      <div className="space-y-6">
        <PageHeader title="Admin users" />
        <div className={panelClass}>
          <EmptyState
            title="Only a super admin can manage admin accounts"
            hint="Creating admins, changing their status, and granting program scopes are super-admin actions. Ask one to make the change for you."
          />
        </div>
      </div>
    );
  }

  return <AdminUsersTable />;
}

function AdminUsersTable() {
  const catalogue = useCatalogue();

  const [q, setQ] = useState("");
  const [role, setRole] = useState("");
  const [status, setStatus] = useState("");
  const [offset, setOffset] = useState(0);

  const [rows, setRows] = useState<AdminUser[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const [creating, setCreating] = useState(false);
  const [scopesFor, setScopesFor] = useState<AdminUser | null>(null);
  const [statusChange, setStatusChange] = useState<{ user: AdminUser; next: UserStatus } | null>(
    null,
  );
  const [statusPending, setStatusPending] = useState(false);

  // Filters reset the page at the source of the change, not in an effect: an
  // effect would render one frame at the stale offset before correcting it.
  function onFilter(apply: () => void) {
    apply();
    setOffset(0);
  }

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const page = await listUsers({
        q: q || undefined,
        role: isRole(role) ? role : undefined,
        status: isStatus(status) ? status : undefined,
        limit: PAGE_SIZE,
        offset,
      });
      setRows(page.items);
      setTotal(page.total);
      setError(null);
    } catch (caught: unknown) {
      setRows([]);
      setTotal(0);
      setError(caught instanceof ApiError ? caught.message : "Could not load admin accounts.");
    } finally {
      setLoading(false);
    }
  }, [q, role, status, offset]);

  useEffect(() => {
    // Wrapped rather than called directly: `load` sets state, and React 19's
    // lint rule counts a synchronous setState in an effect body as a cascade.
    void (async () => {
      await load();
    })();
  }, [load]);

  async function onStatusConfirmed() {
    if (!statusChange) return;
    setStatusPending(true);
    setError(null);
    try {
      await setUserStatus(statusChange.user.id, statusChange.next);
      setNotice(
        `${statusChange.user.email} is now ${STATUS_LABEL[statusChange.next].toLowerCase()}.`,
      );
      setStatusChange(null);
      await load();
    } catch (caught: unknown) {
      setError(caught instanceof ApiError ? caught.message : "Could not change the status.");
    } finally {
      setStatusPending(false);
    }
  }

  const columns: Column<AdminUser>[] = [
    {
      key: "email",
      header: "Email",
      cell: (row) => <span className="font-medium text-gray-900">{row.email}</span>,
    },
    {
      key: "role",
      header: "Role",
      cell: (row) => (
        <StatusPill tone={row.role === "super_admin" ? "info" : "neutral"}>
          {ROLE_LABEL[row.role]}
        </StatusPill>
      ),
    },
    {
      key: "status",
      header: "Status",
      cell: (row) => <StatusPill tone={statusTone(row.status)}>{STATUS_LABEL[row.status]}</StatusPill>,
    },
    {
      key: "last_login",
      header: "Last login",
      cell: (row) => (
        <span className="whitespace-nowrap tabular-nums text-gray-500">
          {row.last_login_at ? formatDate(row.last_login_at) : "Never"}
        </span>
      ),
    },
    {
      key: "created",
      header: "Created",
      cell: (row) => (
        <span className="whitespace-nowrap tabular-nums text-gray-500">
          {formatDate(row.created_at)}
        </span>
      ),
    },
    {
      key: "actions",
      header: "",
      align: "right",
      cell: (row) => (
        <div className="flex justify-end gap-2">
          {/* Scopes only mean anything for a sub-admin: a super admin is
              unrestricted and a student has no admin reach at all. */}
          {row.role === "sub_admin" ? (
            <button
              type="button"
              onClick={() => setScopesFor(row)}
              className="rounded-sm text-sm font-medium text-indigo-500 underline-offset-2 hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-400 focus-visible:ring-offset-2 focus-visible:ring-offset-white"
            >
              Scopes<span className="sr-only"> for {row.email}</span>
            </button>
          ) : null}
          <label className="sr-only" htmlFor={`status-${row.id}`}>
            Change status for {row.email}
          </label>
          <select
            id={`status-${row.id}`}
            value={row.status}
            onChange={(event) => {
              const next = event.target.value;
              if (isStatus(next) && next !== row.status) {
                setStatusChange({ user: row, next });
              }
            }}
            className="rounded-sm border border-lavender-200 bg-white px-2 py-1 text-sm text-gray-900 hover:border-gray-500/50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-400 focus-visible:ring-offset-2 focus-visible:ring-offset-white"
          >
            <option value="active">Active</option>
            <option value="inactive">Inactive</option>
            <option value="suspended">Suspended</option>
          </select>
        </div>
      ),
    },
  ];

  const filtered = Boolean(q || role || status);

  return (
    <div className="space-y-6">
      <PageHeader
        title="Admin users"
        description="Accounts that can manage programs, students, and content."
        action={
          <AdminButton type="button" onClick={() => setCreating(true)}>
            New admin
          </AdminButton>
        }
      />

      {error ? (
        <Banner tone="danger" onDismiss={() => setError(null)}>
          {error}
        </Banner>
      ) : null}
      {notice ? (
        <Banner tone="success" onDismiss={() => setNotice(null)}>
          {notice}
        </Banner>
      ) : null}

      <div className={`${panelClass} flex flex-wrap items-end gap-3 p-4`}>
        <SearchInput value={q} onChange={(next) => onFilter(() => setQ(next))} label="Search admins" placeholder="Email or name..." />
        <div className="w-full sm:w-44">
          <label htmlFor="user-role" className="sr-only">
            Filter by role
          </label>
          <AdminSelect
            id="user-role"
            value={role}
            onChange={(event) => onFilter(() => setRole(event.target.value))}
          >
            <option value="">All roles</option>
            <option value="super_admin">Super admin</option>
            <option value="sub_admin">Sub-admin</option>
            <option value="student">Student</option>
          </AdminSelect>
        </div>
        <div className="w-full sm:w-44">
          <label htmlFor="user-status" className="sr-only">
            Filter by status
          </label>
          <AdminSelect
            id="user-status"
            value={status}
            onChange={(event) => onFilter(() => setStatus(event.target.value))}
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
        loading={loading}
        empty={filtered ? "No accounts match those filters" : "No admin accounts yet"}
        emptyHint={
          filtered
            ? "Try a different search term, role, or status."
            : "Create the first one with New admin."
        }
      />

      <Pagination offset={offset} limit={PAGE_SIZE} total={total} onOffsetChange={setOffset} />

      <NewAdminModal
        open={creating}
        programs={catalogue.programs}
        onClose={() => setCreating(false)}
        onCreated={() => {
          setCreating(false);
          setNotice("Admin account created.");
          void load();
        }}
      />

      <UserScopesModal
        user={scopesFor}
        programs={catalogue.programs}
        onClose={() => setScopesFor(null)}
      />

      <ConfirmDialog
        open={statusChange !== null}
        title={
          statusChange ? `Set ${STATUS_LABEL[statusChange.next].toLowerCase()}?` : "Change status?"
        }
        // The real side effect, not a generic warning: anything other than
        // `active` revokes every `auth_sessions` row for that user.
        consequence={
          statusChange === null
            ? ""
            : statusChange.next === "active"
              ? `${statusChange.user.email} will be able to sign in again. Existing sessions stay revoked, so they must log in fresh.`
              : `${statusChange.user.email} will be signed out everywhere immediately — every one of their sessions is revoked — and cannot sign in until the account is active again.`
        }
        confirmLabel={statusChange ? `Set ${STATUS_LABEL[statusChange.next].toLowerCase()}` : "Confirm"}
        pending={statusPending}
        onConfirm={() => void onStatusConfirmed()}
        onCancel={() => {
          setStatusChange(null);
          // Snap the row's select back to the stored value.
          void load();
        }}
      />
    </div>
  );
}
