import { create } from 'zustand';
import type { AuthUser } from '../types/api';

interface AuthStore {
  user: AuthUser | null;
  setUser: (user: AuthUser | null) => void;
  setSession: (user: AuthUser) => void;
  logout: () => void;
}

export const useAuthStore = create<AuthStore>((set) => ({
  user: null,
  setUser: (user) => set({ user }),
  setSession: (user) => {
    set({ user });
  },
  logout: () => {
    void fetch('/api/v1/auth/logout', {
      method: 'POST',
      credentials: 'same-origin',
      keepalive: true,
    }).catch(() => undefined);
    set({ user: null });
    window.location.href = '/login';
  },
}));
