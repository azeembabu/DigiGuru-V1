/**
 * LSCs — CRUD table for Learner Support Centres.
 */

import * as React from 'react';
import { useForm } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';
import { api, getErrorMessage, isForbiddenError } from '@/lib/api';
import { lscSchema, type LscInput } from '@/lib/validations';
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

interface LscRow {
  id: string;
  name: string;
  code: string;
  location?: string | null;
  status: string;
}

export function Lscs() {
  const [rows, setRows] = React.useState<LscRow[]>([]);
  const [loading, setLoading] = React.useState(true);
  const [forbidden, setForbidden] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const [success, setSuccess] = React.useState<string | null>(null);
  const [editing, setEditing] = React.useState<LscRow | null>(null);
  const [showForm, setShowForm] = React.useState(false);

  const {
    register,
    handleSubmit,
    reset,
    formState: { errors, isSubmitting },
  } = useForm<LscInput>({
    resolver: zodResolver(lscSchema),
    defaultValues: { name: '', code: '', location: '', status: 'ACTIVE' },
  });

  const fetchRows = React.useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await api.get<{ success: true; data: LscRow[] | { items: LscRow[] } }>('/admin/lscs');
      const data = res.data.data as unknown;
      const items = Array.isArray(data) ? (data as LscRow[]) : ((data as { items?: LscRow[] })?.items ?? []);
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
    reset({ name: '', code: '', location: '', status: 'ACTIVE' });
    setShowForm(true);
    setSuccess(null);
  };

  const openEdit = (row: LscRow) => {
    setEditing(row);
    reset({ name: row.name, code: row.code, location: row.location ?? '', status: row.status as 'ACTIVE' | 'INACTIVE' });
    setShowForm(true);
    setSuccess(null);
  };

  const onSubmit = async (values: LscInput) => {
    setError(null);
    setSuccess(null);
    try {
      if (editing) {
        await api.patch(`/admin/lscs/${editing.id}`, values);
        setSuccess(`LSC "${values.name}" updated.`);
      } else {
        await api.post('/admin/lscs', values);
        setSuccess(`LSC "${values.name}" created.`);
      }
      setShowForm(false);
      setEditing(null);
      await fetchRows();
    } catch (err: unknown) {
      if (isForbiddenError(err)) setForbidden(true);
      else setError(getErrorMessage(err));
    }
  };

  const handleDelete = async (row: LscRow) => {
    if (!window.confirm(`Delete LSC "${row.name}" (${row.code})?`)) return;
    setError(null);
    try {
      await api.delete(`/admin/lscs/${row.id}`);
      setSuccess(`LSC "${row.name}" deleted.`);
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
          <h1 className="text-2xl font-bold tracking-tight text-admin-900">Learner Support Centres</h1>
          <p className="mt-1 text-sm text-admin-500">LSC records — code, name, location. Referenced by students at signup.</p>
        </div>
        <Button onClick={openCreate}>New LSC</Button>
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
          <h2 className="text-sm font-semibold text-admin-900">{editing ? 'Edit LSC' : 'New LSC'}</h2>
          <form onSubmit={handleSubmit(onSubmit)} className="mt-4 grid gap-4 sm:grid-cols-2" noValidate>
            <Input label="Name" placeholder="Kochi Centre" error={errors.name?.message} {...register('name')} />
            <Input label="Code" placeholder="LSC-KOCHI-01" error={errors.code?.message} {...register('code')} />
            <div className="sm:col-span-2">
              <Textarea label="Location" rows={2} placeholder="City / address (optional)" error={errors.location?.message} {...register('location')} />
            </div>
            <Select label="Status" error={errors.status?.message} {...register('status')}>
              <option value="ACTIVE">Active</option>
              <option value="INACTIVE">Inactive</option>
            </Select>
            <div className="flex items-end gap-2 sm:col-span-2">
              <Button type="submit" loading={isSubmitting}>
                {editing ? 'Save Changes' : 'Create LSC'}
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
              <TableHeaderCell>Location</TableHeaderCell>
              <TableHeaderCell>Status</TableHeaderCell>
              <TableHeaderCell>Actions</TableHeaderCell>
            </tr>
          </TableHead>
          <TableBody>
            {loading ? (
              <TableSkeleton rows={5} cols={5} />
            ) : rows.length === 0 ? (
              <EmptyRow colSpan={5} message="No LSCs yet." />
            ) : (
              rows.map((r) => (
                <TableRow key={r.id}>
                  <TableCell className="font-medium text-admin-900">{r.name}</TableCell>
                  <TableCell>
                    <Badge variant="neutral">{r.code}</Badge>
                  </TableCell>
                  <TableCell className="max-w-[280px] truncate text-admin-500">{r.location ?? '—'}</TableCell>
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
