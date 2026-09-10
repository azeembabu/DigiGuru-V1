import * as React from 'react';
import { useNavigate } from 'react-router-dom';
import { LogOut } from 'lucide-react';
import { SectionCard } from './SectionCard';
import { Button } from '../ui/Button';
import { api, clearTokens } from '../../lib/api';
import { useAuthStore } from '../../store/authStore';

/**
 * Sign out of this device. The backend revokes the session before the local
 * tokens are cleared, so the refresh token cannot be replayed afterwards.
 */
export function LogoutCard() {
  const navigate = useNavigate();
  const logout = useAuthStore((s) => s.logout);
  const [isLoggingOut, setIsLoggingOut] = React.useState(false);

  async function handleLogout() {
    setIsLoggingOut(true);
    try {
      await api.post('/auth/logout');
    } catch {
      // The session may already be expired — still clear local state.
    } finally {
      clearTokens();
      logout();
      navigate('/login', { replace: true });
    }
  }

  return (
    <SectionCard title="Sign Out" description="End this session on the server and return to the login page.">
      <Button variant="secondary" onClick={handleLogout} loading={isLoggingOut}>
        <LogOut size={16} aria-hidden />
        Log out
      </Button>
    </SectionCard>
  );
}
