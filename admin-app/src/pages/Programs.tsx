/**
 * Programs — CRUD table for academic programs.
 * Endpoints (when backend implements): GET/POST /api/admin/programs, PATCH/DELETE /api/admin/programs/:id
 */

import * as React from 'react';
import { useForm } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';
import { api, getErrorMessage, isForbiddenError } from '@/lib/api';
import { programSchema, type ProgramInput } from '@/lib/validations';
import { Button } from '@/components/ui/Button';
import { Input, Textarea, Select } from '@/components/ui/Input';
import { Badge, StatusBadge } from '@/components/ui/Badge';
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

interface ProgramRow {
  id: string;
  name: string;
  code: string;
  description?: string | null;
  status: string;
  created_at?: string;
}

export function Programs() {
  const [rows, setRows] = React.useState<ProgramRow[]>([]);
  const [loading, setLoading] = React.useState(true);
  const [forbidden, setForbidden] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const [success, setSuccess] = React.useState<string | null>(null);

  const [editing, setEditing] = React.useState<ProgramRow | null>(null);
  const [showForm, setShowForm] = React.useState(false);

  const {
    register,
    handleSubmit,
    reset,
    formState: { errors, isSubmitting },
  } = useForm<ProgramInput>({
    resolver: zodResolver(programSchema),
    defaultValues: { name: '', code: '', description: '', status: 'ACTIVE' },
  });

  const fetchRows = React.useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await api.get<{ success: true; data: ProgramRow[] | { items: ProgramRow[] } }>('/admin/programs');
      const data = res.data.data as unknown;
      const items = Array.isArray(data) ? (data as ProgramRow[]) : ((data as { items?: ProgramRow[] })?.items ?? []);
      setRows(items);
    } catch (err: unknown) {
      if (isForbiddenError(err)) setForbidden(true);
      else setError(getErrorMessage(err));
    } finally {
      setLoading(false);
    }
  }, []);

  React.useEffect(() => {
    fetchRows();
  }, [fetchRows]);

  const openCreate = () => {
    setEditing(null);
    reset({ name: '', code: '', description: '', status: 'ACTIVE' });
    setShowForm(true);
    setSuccess(null);
  };

  const openEdit = (row: ProgramRow) => {
    setEditing(row);
    reset({ name: row.name, code: row.code, description: row.description ?? '', status: row.status as 'ACTIVE' | 'INACTIVE' });
    setShowForm(true);
    setSuccess(null);
  };

  const onSubmit = async (values: ProgramInput) => {
    setError(null);
    setSuccess(null);
    try {
      if (editing) {
        await api.patch(`/admin/programs/${editing.id}`, values);
        setSuccess(`Program "${values.name}" updated.`);
      } else {
        await api.post('/admin/programs', values);
        setSuccess(`Program "${values.name}" created.`);
      }
      setShowForm(false);
      setEditing(null);
      await fetchRows();
    } catch (err: unknown) {
      if (isForbiddenError(err)) setForbidden(true);
      else setError(getErrorMessage(err));
    }
  };

  const handleDelete = async (row: ProgramRow) => {
    if (!window.confirm(`Delete program "${row.name}" (${row.code})? This cannot be undone.`)) return;
    setError(null);
    try {
      await api.delete(`/admin/programs/${row.id}`);
      setSuccess(`Program "${row.name}" deleted.`);
      await fetchRows();
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
          <h1 className="text-2xl font-bold tracking-tight text-admin-900">Programs</h1>
          <p className="mt-1 text-sm text-admin-500">Academic programs — e.g. BA Malayalam, BCom, BA English.</p>
        </div>
        <Button onClick={openCreate}>New Program</Button>
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
          <h2 className="text-sm font-semibold text-admin-900">{editing ? 'Edit Program' : 'New Program'}</h2>
          <form onSubmit={handleSubmit(onSubmit)} className="mt-4 grid gap-4 sm:grid-cols-2" noValidate>
            <Input label="Name" placeholder="BA Malayalam" error={errors.name?.message} {...register('name')} />
            <Input label="Code" placeholder="BA_ML" error={errors.code?.message} {...register('code')} />
            <div className="sm:col-span-2">
              <Textarea label="Description" rows={3} placeholder="Optional description…" error={errors.description?.message} {...register('description')} />
            </div>
            <Select label="Status" error={errors.status?.message} {...register('status')}>
              <option value="ACTIVE">Active</option>
              <option value="INACTIVE">Inactive</option>
            </Select>
            <div className="flex items-end gap-2 sm:col-span-2">
              <Button type="submit" loading={isSubmitting}>
                {editing ? 'Save Changes' : 'Create Program'}
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
              <TableHeaderCell>Code</TableHeaderCell>
              <TableHeaderCell>Description</TableHeaderCell>
              <TableHeaderCell>Status</TableHeaderCell>
              <TableHeaderCell>Actions</TableHeaderCell>
            </tr>
          </TableHead>
          <TableBody>
            {loading ? (
              <TableSkeleton rows={5} cols={5} />
            ) : rows.length === 0 ? (
              <EmptyRow colSpan={5} message="No programs yet. Create the first one above." />
            ) : (
              rows.map((r) => (
                <TableRow key={r.id}>
                  <TableCell className="font-medium text-admin-900">{r.name}</TableCell>
                  <TableCell>
                    <Badge variant="neutral">{r.code}</Badge>
                  </TableCell>
                  <TableCell className="max-w-[360px] truncate text-admin-500">{r.description ?? '—'}</TableCell>
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
