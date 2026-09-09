import * as React from 'react';
import { Navigate, Route, Routes, useLocation } from 'react-router-dom';
import { useAuthStore } from './store/authStore';
import { Layout } from './components/Layout';
import { ProtectedRoute } from './components/ProtectedRoute';

import LandingPage from './pages/LandingPage';
import StudentLogin from './pages/StudentLogin';
import StudentSignup from './pages/StudentSignup';
import ForgotPassword from './pages/ForgotPassword';
import ResetPassword from './pages/ResetPassword';
import StudentDashboard from './pages/StudentDashboard';
import StudentProfile from './pages/StudentProfile';
import StudentCourses from './pages/StudentCourses';
import StudentSettings from './pages/StudentSettings';

/**
 * CRITICAL: No Admin surface exists in this app.
 * Any attempt to access /admin* is redirected to /login.
 */
function AdminBlock() {
  return <Navigate to="/login" replace />;
}

function PublicOnly({ children }: { children: React.ReactNode }) {
  const { isAuthenticated, isHydrated } = useAuthStore();
  const location = useLocation();
  // Don't redirect until hydration completes
  if (!isHydrated) return <>{children}</>;
  if (isAuthenticated) {
    const dest = (location.state as { from?: { pathname: string } } | null)?.from?.pathname ?? '/student/dashboard';
    return <Navigate to={dest} replace />;
  }
  return <>{children}</>;
}

export default function App() {
  const hydrate = useAuthStore((s) => s.hydrate);

  React.useEffect(() => {
    hydrate();
  }, [hydrate]);

  return (
    <Routes>
      {/* Public auth routes — no Layout chrome, each page owns its background */}
      <Route
        path="/login"
        element={
          <PublicOnly>
            <StudentLogin />
          </PublicOnly>
        }
      />
      <Route
        path="/signup"
        element={
          <PublicOnly>
            <StudentSignup />
          </PublicOnly>
        }
      />
      <Route path="/forgot-password" element={<ForgotPassword />} />
      <Route path="/reset-password" element={<ResetPassword />} />

      {/* Protected student area — wrapped in Layout */}
      <Route
        path="/student/dashboard"
        element={
          <ProtectedRoute>
            <Layout>
              <StudentDashboard />
            </Layout>
          </ProtectedRoute>
        }
      />
      <Route
        path="/student/profile"
        element={
          <ProtectedRoute>
            <Layout>
              <StudentProfile />
            </Layout>
          </ProtectedRoute>
        }
      />
      <Route
        path="/student/courses"
        element={
          <ProtectedRoute>
            <Layout>
              <StudentCourses />
            </Layout>
          </ProtectedRoute>
        }
      />
      <Route
        path="/student/settings"
        element={
          <ProtectedRoute>
            <Layout>
              <StudentSettings />
            </Layout>
          </ProtectedRoute>
        }
      />

      {/* Admin block — enforce at route level. No Admin UI exists. */}
      <Route path="/admin" element={<AdminBlock />} />
      <Route path="/admin/*" element={<AdminBlock />} />

      {/* Landing page — public marketing page */}
      <Route path="/" element={<LandingPage />} />

      {/* Defaults */}
      <Route path="*" element={<Navigate to="/" replace />} />
    </Routes>
  );
}
