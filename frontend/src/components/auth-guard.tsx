import { useEffect } from 'react';
import { Navigate } from 'react-router-dom';
import { Spinner } from '@blueprintjs/core';
import { getMe } from '../api/auth';
import { useAuthStore } from '../stores/auth-store';

export const AuthGuard = ({ children }: { children: React.ReactNode }) => {
  const token = useAuthStore((s) => s.token);
  const user = useAuthStore((s) => s.user);
  const setUser = useAuthStore((s) => s.setUser);
  const logout = useAuthStore((s) => s.logout);

  useEffect(() => {
    if (!token || user) return;

    let cancelled = false;

    getMe()
      .then((me) => {
        if (cancelled) return;
        setUser(me);
      })
      .catch(() => {
        if (cancelled) return;
        logout();
      });

    return () => {
      cancelled = true;
    };
  }, [logout, setUser, token, user]);

  if (!token) {
    return <Navigate to="/login" replace />;
  }

  if (!user) {
    return (
      <div
        style={{ display: 'flex', justifyContent: 'center', alignItems: 'center', height: '100%' }}
      >
        <Spinner size={40} />
      </div>
    );
  }

  return <>{children}</>;
};
