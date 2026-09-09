/**
 * Students — table fetching GET /api/admin/students
 * Columns: Name, Roll No, Program, Sem, LSC, Status, Actions. Search + filter.
 */

import * as React from 'react';
import { api, getErrorMessage, isForbiddenError } from '@/lib/api';
import { Input, Select } from '@/components/ui/Input';
import { Button } from '@/components/ui/Button';
import { Badge, StatusBadge } from '@/components/ui/Badge';
import { AccessDenied } from '@/components/ProtectedAdminRoute';
import {
  TableWrapper,
  Table,
  TableHead,
  TableBody,
  TableRow,
  TableHeaderCell,
  TableCell,
  EmptyRow,
  TableSkeleton,
} from '@/components/ui/Table';

interface StudentRow {
  id: string;
  full_name: string;
  roll_number: string;
  phone_number: string;
  program_id: string;
  semester_id: string;
  lsc_id: string;
  // Enriched by backend when available
  program?: { name: string; code: string } | null;
  semester?: { name: string; semester_number: number } | null;
  lsc?: { name: string; code: string } | null;
  user?: { email: string; status: string } | null;
  status?: string;
  created_at?: string;
}

type StudentsResponse =
  | { success: true; data: StudentRow[]; meta?: { total: number; page: number; limit: number } }
  | { success: true; data: { items: StudentRow[]; total: number } };

function extractRows(body: StudentsResponse): StudentRow[] {
  const d = body.data as unknown;
  if (Array.isArray(d)) return d as StudentRow[];
  if (d && typeof d === 'object' && 'items' in (d as Record<string, unknown>)) {
    return ((d as { items: StudentRow[] }).items ?? []) as StudentRow[];
  }
  return [];
}

export function Students() {
  const [rows, setRows] = React.useState<StudentRow[]>([]);
  const [total, setTotal] = React.useState<number | null>(null);
  const [loading, setLoading] = React.useState(true);
  const [forbidden, setForbidden] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);

  const [search, setSearch] = React.useState('');
  const [statusFilter, setStatusFilter] = React.useState<string>('ALL');
  const [page, setPage] = React.useState(1);
  const limit = 20;

  const debouncedSearch = useDebounce(search, 350);

  const fetchStudents = React.useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const params: Record<string, string | number> = { page, limit };
      if (debouncedSearch.trim()) params.search = debouncedSearch.trim();
      if (statusFilter !== 'ALL') params.status = statusFilter;

      const res = await api.get<StudentsResponse>('/admin/students', { params });
      const body = res.data as StudentsResponse;
      const items = extractRows(body);
      setRows(items);

      const meta = (body as { meta?: { total?: number } }).meta;
      if (meta?.total != null) setTotal(meta.total);
      else if ((body.data as { total?: number })?.total != null) setTotal((body.data as { total: number }).total);
      else setTotal(items.length);
    } catch (err: unknown) {
      if (isForbiddenError(err)) {
        setForbidden(true);
      } else {
        setError(getErrorMessage(err));
      }
    } finally {
      setLoading(false);
    }
  }, [debouncedSearch, statusFilter, page]);

  React.useEffect(() => {
    fetchStudents();
  }, [fetchStudents]);

  // Reset to page 1 when search/filter changes
  React.useEffect(() => {
    setPage(1);
  }, [debouncedSearch, statusFilter]);

  const filteredRows = React.useMemo(() => {
    if (!search.trim() && statusFilter === 'ALL') return rows;
    // Client-side fallback when backend doesn't filter — keep it light.
    return rows.filter((r) => {
      const q = search.trim().toLowerCase();
      const matchesSearch =
        !q ||
        r.full_name.toLowerCase().includes(q) ||
        r.roll_number.toLowerCase().includes(q) ||
        r.phone_number.includes(q);
      const s = (r.user?.status ?? r.status ?? 'ACTIVE').toUpperCase();
      const matchesStatus = statusFilter === 'ALL' || s === statusFilter;
      return matchesSearch && matchesStatus;
    });
  }, [rows, search, statusFilter]);

  if (forbidden) return <AccessDenied />;

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold tracking-tight text-admin-900">Students</h1>
          <p className="mt-1 text-sm text-admin-500">
            {total != null ? `${total} total` : 'All registered students'} — via <code className="rounded bg-admin-100 px-1 py-0.5 font-mono text-xs">GET /api/admin/students</code>
          </p>
        </div>
        <Button variant="secondary" onClick={fetchStudents} disabled={loading}>
          Refresh
        </Button>
      </div>

      {error && (
        <div role="alert" className="rounded-lg border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-800">
          {error}
        </div>
      )}

      {/* Filters */}
      <div className="flex flex-col gap-3 sm:flex-row sm:items-end">
        <div className="flex-1 sm:max-w-sm">
          <Input
            label="Search"
            placeholder="Name, roll number, or phone…"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
        </div>
        <div className="w-full sm:w-48">
          <Select label="Status" value={statusFilter} onChange={(e) => setStatusFilter(e.target.value)}>
            <option value="ALL">All statuses</option>
            <option value="ACTIVE">Active</option>
            <option value="INACTIVE">Inactive</option>
            <option value="SUSPENDED">Suspended</option>
          </Select>
        </div>
      </div>

      {/* Table */}
      <TableWrapper>
        <Table>
          <TableHead>
            <tr>
              <TableHeaderCell>Name</TableHeaderCell>
              <TableHeaderCell>Roll No</TableHeaderCell>
              <TableHeaderCell>Program</TableHeaderCell>
              <TableHeaderCell>Sem</TableHeaderCell>
              <TableHeaderCell>LSC</TableHeaderCell>
              <TableHeaderCell>Status</TableHeaderCell>
              <TableHeaderCell>Actions</TableHeaderCell>
            </tr>
          </TableHead>
          <TableBody>
            {loading ? (
              <TableSkeleton rows={6} cols={7} />
            ) : filteredRows.length === 0 ? (
              <EmptyRow colSpan={7} message={search || statusFilter !== 'ALL' ? 'No students match your filters.' : 'No students found.'} />
            ) : (
              filteredRows.map((s) => {
                const status = (s.user?.status ?? s.status ?? 'ACTIVE') as string;
                return (
                  <TableRow key={s.id}>
                    <TableCell>
                      <div className="min-w-[160px]">
                        <p className="font-medium text-admin-900">{s.full_name}</p>
                        <p className="font-mono text-xs text-admin-500">{s.phone_number}</p>
                      </div>
                    </TableCell>
                    <TableCell className="whitespace-nowrap font-mono text-xs font-medium">{s.roll_number}</TableCell>
                    <TableCell className="whitespace-nowrap">
                      {s.program ? (
                        <span>
                          {s.program.name} <span className="font-mono text-xs text-admin-400">{s.program.code}</span>
                        </span>
                      ) : (
                        <span className="font-mono text-xs text-admin-400">{s.program_id.slice(0, 8)}…</span>
                      )}
                    </TableCell>
                    <TableCell className="whitespace-nowrap">
                      {s.semester ? s.semester.name : <span className="font-mono text-xs text-admin-400">{s.semester_id.slice(0, 8)}…</span>}
                    </TableCell>
                    <TableCell className="whitespace-nowrap">
                      {s.lsc ? (
                        <Badge variant="neutral">{s.lsc.code}</Badge>
                      ) : (
                        <span className="font-mono text-xs text-admin-400">{s.lsc_id.slice(0, 8)}…</span>
                      )}
                    </TableCell>
                    <TableCell>
                      <StatusBadge status={status} />
                    </TableCell>
                    <TableCell>
                      <div className="flex gap-1">
                        <Button variant="ghost" size="sm" title="View details" onClick={() => alert(`Student: ${s.full_name} (${s.roll_number})`)}>
                          View
                        </Button>
                      </div>
                    </TableCell>
                  </TableRow>
                );
              })
            )}
          </TableBody>
        </Table>
      </TableWrapper>

      {/* Pagination */}
      <div className="flex items-center justify-between">
        <p className="text-sm text-admin-500">
          Page {page} {total != null ? `· ${total} total` : ''}
        </p>
        <div className="flex gap-2">
          <Button variant="secondary" size="sm" disabled={page <= 1 || loading} onClick={() => setPage((p) => Math.max(1, p - 1))}>
            Previous
          </Button>
          <Button variant="secondary" size="sm" disabled={loading || (total != null && page * limit >= total)} onClick={() => setPage((p) => p + 1)}>
            Next
          </Button>
        </div>
      </div>
    </div>
  );
}

function useDebounce<T>(value: T, delay: number): T {
  const [debounced, setDebounced] = React.useState(value);
  React.useEffect(() => {
    const id = window.setTimeout(() => setDebounced(value), delay);
    return () => window.clearTimeout(id);
  }, [value, delay]);
  return debounced;
}
