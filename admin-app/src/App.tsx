import * as React from 'react';
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { AdminLayout } from '@/components/AdminLayout';
import { ProtectedAdminRoute } from '@/components/ProtectedAdminRoute';
import { AdminLogin } from '@/pages/AdminLogin';
import { Dashboard } from '@/pages/Dashboard';
import { Students } from '@/pages/Students';
import { Programs } from '@/pages/Programs';
import { Semesters } from '@/pages/Semesters';
import { Lscs } from '@/pages/Lscs';
import { Settings } from '@/pages/Settings';
import { useAuthStore } from '@/store/authStore';

function NotFound() {
  return (
    <div className="flex min-h-[50vh] flex-col items-center justify-center px-6 py-16 text-center">
      <p className="font-mono text-xs tracking-widest text-admin-400">404</p>
      <h1 className="mt-2 text-xl font-semibold text-admin-900">Page not found</h1>
      <p className="mt-2 text-sm text-admin-500">The page you are looking for does not exist.</p>
      <a href="/admin/dashboard" className="mt-6 inline-flex h-9 items-center rounded-md bg-indigo-600 px-4 text-sm font-medium text-white hover:bg-indigo-700">
        Go to Dashboard
      </a>
    </div>
  );
}

function HydrateAuth({ children }: { children: React.ReactNode }) {
  const hydrate = useAuthStore((s) => s.hydrate);
  const hydrated = React.useRef(false);

  if (!hydrated.current) {
    hydrated.current = true;
    hydrate();
  }

  return <>{children}</>;
}

export default function App() {
  return (
    <BrowserRouter>
      <HydrateAuth>
        <Routes>
          {/* Public — admin login */}
          <Route path="/admin/login" element={<AdminLogin />} />

          {/* Protected — all /admin/* except /admin/login */}
          <Route
            element={
              <ProtectedAdminRoute>
                <AdminLayout />
              </ProtectedAdminRoute>
            }
          >
            <Route path="/admin/dashboard" element={<Dashboard />} />
            <Route path="/admin/students" element={<Students />} />
            <Route path="/admin/programs" element={<Programs />} />
            <Route path="/admin/semesters" element={<Semesters />} />
            <Route path="/admin/lsc" element={<Lscs />} />
            <Route path="/admin/settings" element={<Settings />} />
          </Route>

          {/* Root → redirect to admin login/dashboard based on auth would be handled by login; keep explicit */}
          <Route path="/" element={<Navigate to="/admin/login" replace />} />
          <Route path="/admin" element={<Navigate to="/admin/dashboard" replace />} />
          <Route path="*" element={<NotFound />} />
        </Routes>
      </HydrateAuth>
    </BrowserRouter>
  );
}
