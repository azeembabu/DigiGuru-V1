/**
 * Semesters — CRUD table for semesters (scoped to a program).
 */

import * as React from 'react';
import { useForm } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';
import { api, getErrorMessage, isForbiddenError } from '@/lib/api';
import { semesterSchema, type SemesterInput } from '@/lib/validations';
import { Button } from '@/components/ui/Button';
import { Input, Select } from '@/components/ui/Input';
import { StatusBadge } from '@/components/ui/Badge';
import { Card } from '@/components/ui/Card';
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

interface SemesterRow {
  id: string;
  program_id: string;
  semester_number: number;
  name: string;
  status: string;
  program?: { name: string; code: string } | null;
}

interface ProgramOption {
  id: string;
  name: string;
  code: string;
}

export function Semesters() {
  const [rows, setRows] = React.useState<SemesterRow[]>([]);
  const [programs, setPrograms] = React.useState<ProgramOption[]>([]);
  const [loading, setLoading] = React.useState(true);
  const [forbidden, setForbidden] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const [success, setSuccess] = React.useState<string | null>(null);
  const [editing, setEditing] = React.useState<SemesterRow | null>(null);
  const [showForm, setShowForm] = React.useState(false);

  const {
    register,
    handleSubmit,
    reset,
    formState: { errors, isSubmitting },
  } = useForm<SemesterInput>({
    resolver: zodResolver(semesterSchema),
    defaultValues: { program_id: '', semester_number: 1, name: '', status: 'ACTIVE' },
  });

  const fetchData = React.useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [semRes, progRes] = await Promise.allSettled([
        api.get<{ success: true; data: SemesterRow[] | { items: SemesterRow[] } }>('/admin/semesters'),
        api.get<{ success: true; data: ProgramOption[] | { items: ProgramOption[] } }>('/admin/programs'),
      ]);

      if (semRes.status === 'rejected' && isForbiddenError(semRes.reason)) {
        setForbidden(true);
        return;
      }
      if (progRes.status === 'rejected' && isForbiddenError(progRes.reason)) {
        setForbidden(true);
        return;
      }

      if (semRes.status === 'fulfilled') {
        const d = semRes.value.data.data as unknown;
        const items = Array.isArray(d) ? (d as SemesterRow[]) : ((d as { items?: SemesterRow[] })?.items ?? []);
        setRows(items);
      } else if (semRes.status === 'rejected') {
        setError(getErrorMessage(semRes.reason));
      }

      if (progRes.status === 'fulfilled') {
        const d = progRes.value.data.data as unknown;
        const items = Array.isArray(d) ? (d as ProgramOption[]) : ((d as { items?: ProgramOption[] })?.items ?? []);
        setPrograms(items);
      }
    } catch (err: unknown) {
      if (isForbiddenError(err)) setForbidden(true);
      else setError(getErrorMessage(err));
    } finally {
      setLoading(false);
    }
  }, []);

  React.useEffect(() => {
    fetchData();
  }, [fetchData]);

  const openCreate = () => {
    setEditing(null);
    reset({ program_id: programs[0]?.id ?? '', semester_number: 1, name: '', status: 'ACTIVE' });
    setShowForm(true);
    setSuccess(null);
  };

  const openEdit = (row: SemesterRow) => {
    setEditing(row);
    reset({
      program_id: row.program_id,
      semester_number: row.semester_number,
      name: row.name,
      status: row.status as 'ACTIVE' | 'INACTIVE',
    });
    setShowForm(true);
    setSuccess(null);
  };

  const onSubmit = async (values: SemesterInput) => {
    setError(null);
    setSuccess(null);
    try {
      if (editing) {
        await api.patch(`/admin/semesters/${editing.id}`, values);
        setSuccess(`Semester "${values.name}" updated.`);
      } else {
        await api.post('/admin/semesters', values);
        setSuccess(`Semester "${values.name}" created.`);
      }
      setShowForm(false);
      setEditing(null);
      await fetchData();
    } catch (err: unknown) {
      if (isForbiddenError(err)) setForbidden(true);
      else setError(getErrorMessage(err));
    }
  };

  const handleDelete = async (row: SemesterRow) => {
    if (!window.confirm(`Delete semester "${row.name}"?`)) return;
    setError(null);
    try {
      await api.delete(`/admin/semesters/${row.id}`);
      setSuccess(`Semester "${row.name}" deleted.`);
      await fetchData();
    } catch (err: unknown) {
      if (isForbiddenError(err)) setForbidden(true);
      else setError(getErrorMessage(err));
    }
  };

  if (forbidden) return <AccessDenied />;

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold tracking-tight text-admin-900">Semesters</h1>
          <p className="mt-1 text-sm text-admin-500">Semesters belong to a program. Semester number is unique per program.</p>
        </div>
        <Button onClick={openCreate}>New Semester</Button>
      </div>

      {error && (
        <div role="alert" className="rounded-lg border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-800">
          {error}
        </div>
      )}
      {success && (
        <div role="status" className="rounded-lg border border-emerald-200 bg-emerald-50 px-4 py-3 text-sm text-emerald-800">
          {success}
        </div>
      )}

      {showForm && (
        <Card>
          <h2 className="text-sm font-semibold text-admin-900">{editing ? 'Edit Semester' : 'New Semester'}</h2>
          <form onSubmit={handleSubmit(onSubmit)} className="mt-4 grid gap-4 sm:grid-cols-2" noValidate>
            <Select label="Program" error={errors.program_id?.message} {...register('program_id')}>
              <option value="">Select a program</option>
              {programs.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name} ({p.code})
                </option>
              ))}
            </Select>
            <Input label="Semester Number" type="number" min={1} max={12} error={errors.semester_number?.message} {...register('semester_number')} />
            <Input label="Name" placeholder="Semester 1" error={errors.name?.message} {...register('name')} />
            <Select label="Status" error={errors.status?.message} {...register('status')}>
              <option value="ACTIVE">Active</option>
              <option value="INACTIVE">Inactive</option>
            </Select>
            <div className="flex items-end gap-2 sm:col-span-2">
              <Button type="submit" loading={isSubmitting}>
                {editing ? 'Save Changes' : 'Create Semester'}
              </Button>
              <Button type="button" variant="secondary" onClick={() => { setShowForm(false); setEditing(null); }}>
                Cancel
              </Button>
            </div>
          </form>
        </Card>
      )}

      <TableWrapper>
        <Table>
          <TableHead>
            <tr>
              <TableHeaderCell>Name</TableHeaderCell>
              <TableHeaderCell>#</TableHeaderCell>
              <TableHeaderCell>Program</TableHeaderCell>
              <TableHeaderCell>Status</TableHeaderCell>
              <TableHeaderCell>Actions</TableHeaderCell>
            </tr>
          </TableHead>
          <TableBody>
            {loading ? (
              <TableSkeleton rows={5} cols={5} />
            ) : rows.length === 0 ? (
              <EmptyRow colSpan={5} message="No semesters yet." />
            ) : (
              rows.map((r) => (
                <TableRow key={r.id}>
                  <TableCell className="font-medium text-admin-900">{r.name}</TableCell>
                  <TableCell className="font-mono text-xs">{r.semester_number}</TableCell>
                  <TableCell className="text-sm">
                    {r.program ? `${r.program.name} (${r.program.code})` : <span className="font-mono text-xs text-admin-400">{r.program_id.slice(0, 8)}…</span>}
                  </TableCell>
                  <TableCell>
                    <StatusBadge status={r.status} />
                  </TableCell>
                  <TableCell>
                    <div className="flex gap-1">
                      <Button variant="ghost" size="sm" onClick={() => openEdit(r)}>
                        Edit
                      </Button>
                      <Button variant="ghost" size="sm" onClick={() => handleDelete(r)}>
                        Delete
                      </Button>
                    </div>
                  </TableCell>
                </TableRow>
              ))
            )}
          </TableBody>
        </Table>
      </TableWrapper>
    </div>
  );
}
