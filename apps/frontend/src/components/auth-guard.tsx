import { useEffect, useState } from 'react';
import { Navigate } from 'react-router-dom';
import { Spinner } from '@blueprintjs/core';
import { getMe } from '../api/auth';
import { useAuthStore } from '../stores/auth-store';

export const AuthGuard = ({ children }: { children: React.ReactNode }) => {
  const user = useAuthStore((s) => s.user);
  const setUser = useAuthStore((s) => s.setUser);
  const [checked, setChecked] = useState(false);

  useEffect(() => {
    if (user) return;

    let cancelled = false;

    getMe()
      .then((me) => {
        if (cancelled) return;
        setUser(me);
        setChecked(true);
      })
      .catch(() => {
        if (cancelled) return;
        setChecked(true);
      });

    return () => {
      cancelled = true;
    };
  }, [setUser, user]);

  if (!user && !checked) {
    return (
      <div
        style={{ display: 'flex', justifyContent: 'center', alignItems: 'center', height: '100%' }}
      >
        <Spinner size={40} />
      </div>
    );
  }

  if (!user) {
    return <Navigate to="/login" replace />;
  }

  return <>{children}</>;
};
