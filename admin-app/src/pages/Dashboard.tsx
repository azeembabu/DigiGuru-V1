/**
 * Admin Dashboard — stats cards + recent audit logs.
 * Gracefully handles missing /api/admin/* endpoints (shows placeholders).
 */

import * as React from 'react';
import { api, getErrorMessage, isForbiddenError } from '@/lib/api';
import { Card, StatCard } from '@/components/ui/Card';
import { Badge } from '@/components/ui/Badge';
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

interface DashboardStats {
  totalStudents: number;
  activeSessions: number;
  programs: number;
  lscs: number;
}

interface AuditLog {
  id: string;
  action: string;
  user_id?: string | null;
  ip_address?: string | null;
  created_at: string;
  metadata?: Record<string, unknown> | null;
}

function formatDate(value: string): string {
  try {
    return new Date(value).toLocaleString();
  } catch {
    return value;
  }
}

export function Dashboard() {
  const [stats, setStats] = React.useState<DashboardStats | null>(null);
  const [logs, setLogs] = React.useState<AuditLog[]>([]);
  const [loading, setLoading] = React.useState(true);
  const [forbidden, setForbidden] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);

  React.useEffect(() => {
    let cancelled = false;

    async function load() {
      setLoading(true);
      setError(null);
      setForbidden(false);

      // Dashboard tries admin-specific endpoints. If they are not yet
      // implemented, we fall back to generic ones so the page still renders.
      try {
        const [statsRes, logsRes] = await Promise.allSettled([
          api.get<{ success: true; data: DashboardStats }>('/admin/stats'),
          api.get<{ success: true; data: AuditLog[] }>('/admin/audit-logs', { params: { limit: 10 } }),
        ]);

        if (cancelled) return;

        // Forbidden on either call → render Access Denied.
        const anyForbidden =
          (statsRes.status === 'rejected' && isForbiddenError((statsRes as PromiseRejectedResult).reason)) ||
          (logsRes.status === 'rejected' && isForbiddenError((logsRes as PromiseRejectedResult).reason));

        if (anyForbidden) {
          setForbidden(true);
          setLoading(false);
          return;
        }

        if (statsRes.status === 'fulfilled') {
          setStats(statsRes.value.data.data);
        } else {
          // Stats endpoint missing — try a lighter fallback so cards are not empty.
          try {
            const fallback = await api.get<{ success: true; data: { total: number } }>('/admin/students', {
              params: { limit: 1 },
            });
            const total = (fallback.data as unknown as { meta?: { total?: number } })?.meta?.total ?? 0;
            setStats((prev) => prev ?? { totalStudents: total, activeSessions: 0, programs: 0, lscs: 0 });
          } catch {
            // remain null — cards show "—"
          }
        }

        if (logsRes.status === 'fulfilled') {
          const data = logsRes.value.data.data;
          setLogs(Array.isArray(data) ? data : []);
        } else {
          // Try generic audit-logs without /admin prefix as fallback.
          try {
            const fb = await api.get<{ success: true; data: AuditLog[] }>('/admin/audit-logs', {
              params: { limit: 10 },
            });
            const data = fb.data.data;
            setLogs(Array.isArray(data) ? data : []);
          } catch {
            // leave empty
          }
        }
      } catch (err: unknown) {
        if (isForbiddenError(err)) {
          setForbidden(true);
        } else {
          setError(getErrorMessage(err));
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    load();
    return () => {
      cancelled = true;
    };
  }, []);

  if (forbidden) return <AccessDenied />;

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold tracking-tight text-admin-900">Dashboard</h1>
        <p className="mt-1 text-sm text-admin-500">Overview of students, programs, and recent activity.</p>
      </div>

      {error && (
        <div role="alert" className="rounded-lg border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-800">
          Could not load some dashboard data: {error}
        </div>
      )}

      {/* Stats */}
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <StatCard
          label="Total Students"
          value={loading ? '…' : (stats?.totalStudents ?? '—')}
          hint="All registered students"
          icon={
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden>
              <path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2" />
              <circle cx="9" cy="7" r="4" />
            </svg>
          }
        />
        <StatCard
          label="Active Sessions"
          value={loading ? '…' : (stats?.activeSessions ?? '—')}
          hint="Currently signed in"
          icon={
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden>
              <circle cx="12" cy="12" r="10" />
              <polyline points="12 6 12 12 16 14" />
            </svg>
          }
        />
        <StatCard
          label="Programs"
          value={loading ? '…' : (stats?.programs ?? '—')}
          hint="Academic programs"
          icon={
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden>
              <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20" />
              <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z" />
            </svg>
          }
        />
        <StatCard
          label="LSCs"
          value={loading ? '…' : (stats?.lscs ?? '—')}
          hint="Learner Support Centres"
          icon={
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden>
              <path d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
            </svg>
          }
        />
      </div>

      {/* Audit logs */}
      <Card padding="none">
        <div className="flex items-center justify-between border-b border-admin-100 px-5 py-4">
          <div>
            <h2 className="text-sm font-semibold text-admin-900">Recent audit logs</h2>
            <p className="mt-1 text-xs text-admin-500">Latest admin and system actions.</p>
          </div>
          <Badge variant="neutral">{logs.length} entries</Badge>
        </div>

        <TableWrapper className="rounded-none border-0 shadow-none">
          <Table>
            <TableHead>
              <tr>
                <TableHeaderCell>Time</TableHeaderCell>
                <TableHeaderCell>Action</TableHeaderCell>
                <TableHeaderCell>User</TableHeaderCell>
                <TableHeaderCell>IP</TableHeaderCell>
              </tr>
            </TableHead>
            <TableBody>
              {loading ? (
                <TableSkeleton rows={5} cols={4} />
              ) : logs.length === 0 ? (
                <EmptyRow colSpan={4} message="No audit logs yet. Actions will appear here once the backend records them." />
              ) : (
                logs.map((log) => (
                  <TableRow key={log.id}>
                    <TableCell className="whitespace-nowrap font-mono text-xs">{formatDate(log.created_at)}</TableCell>
                    <TableCell>
                      <Badge variant="neutral">{log.action}</Badge>
                    </TableCell>
                    <TableCell className="max-w-[180px] truncate font-mono text-xs">{log.user_id ?? '—'}</TableCell>
                    <TableCell className="font-mono text-xs">{log.ip_address ?? '—'}</TableCell>
                  </TableRow>
                ))
              )}
            </TableBody>
          </Table>
        </TableWrapper>
      </Card>
    </div>
  );
}
