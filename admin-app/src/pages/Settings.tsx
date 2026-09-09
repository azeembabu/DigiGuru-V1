import { Card } from '@/components/ui/Card';
import { Badge } from '@/components/ui/Badge';
import { useAuthStore } from '@/store/authStore';

export function Settings() {
  const { user } = useAuthStore();

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold tracking-tight text-admin-900">Settings</h1>
        <p className="mt-1 text-sm text-admin-500">Admin session and environment information.</p>
      </div>

      <Card>
        <h2 className="text-sm font-semibold text-admin-900">Signed in as</h2>
        <dl className="mt-4 grid gap-3 text-sm sm:grid-cols-2">
          <div>
            <dt className="text-xs font-medium uppercase tracking-widest text-admin-500">Name</dt>
            <dd className="mt-1 font-medium text-admin-900">{user?.fullName ?? '—'}</dd>
          </div>
          <div>
            <dt className="text-xs font-medium uppercase tracking-widest text-admin-500">Email</dt>
            <dd className="mt-1 font-mono text-admin-900">{user?.email ?? '—'}</dd>
          </div>
          <div>
            <dt className="text-xs font-medium uppercase tracking-widest text-admin-500">Role</dt>
            <dd className="mt-1">
              <Badge variant="default">{user?.role ?? '—'}</Badge>
            </dd>
          </div>
          <div>
            <dt className="text-xs font-medium uppercase tracking-widest text-admin-500">User ID</dt>
            <dd className="mt-1 font-mono text-xs text-admin-600">{user?.id ?? '—'}</dd>
          </div>
        </dl>
        <p className="mt-6 text-xs leading-5 text-admin-500">
          Admin accounts are created via the backend (seed / admin API). There is no public signup for administrators.
        </p>
      </Card>

      <Card>
        <h2 className="text-sm font-semibold text-admin-900">Separation guarantee</h2>
        <p className="mt-2 text-sm leading-6 text-admin-500">
          The Admin app is a <strong className="font-semibold text-admin-700">separate Vite app</strong> running on port <code className="rounded bg-admin-100 px-1.5 py-0.5 font-mono text-xs">5174</code> and calling <code className="rounded bg-admin-100 px-1.5 py-0.5 font-mono text-xs">/api/admin/*</code>. The Student app runs on port{' '}
          <code className="rounded bg-admin-100 px-1.5 py-0.5 font-mono text-xs">5173</code> and has no Admin surface — no link, no route, no import — that could leak the admin surface. In production the two apps live on
          different domains (e.g. <code className="font-mono text-xs">admin.digiguru.com</code> vs <code className="font-mono text-xs">student.digiguru.com</code>). A <code className="font-mono text-xs">STUDENT</code> JWT hitting any{' '}
          <code className="font-mono text-xs">/api/admin/*</code> endpoint receives <code className="font-mono text-xs">403 Forbidden</code> and the UI renders an explicit “Access Denied — Admin only” page.
        </p>
      </Card>
    </div>
  );
}
