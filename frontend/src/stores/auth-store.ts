import { create } from 'zustand';
import type { AuthUser } from '../types/api';

interface AuthStore {
  token: string | null;
  user: AuthUser | null;
  setToken: (token: string | null) => void;
  setUser: (user: AuthUser | null) => void;
  setSession: (token: string, user: AuthUser) => void;
  logout: () => void;
}

export const useAuthStore = create<AuthStore>((set) => ({
  token: null,
  user: null,
  setToken: (token) => {
    set({ token, ...(token ? {} : { user: null }) });
  },
  setUser: (user) => set({ user }),
  setSession: (token, user) => {
    set({ token, user });
  },
  logout: () => {
    void fetch('/api/v1/auth/logout', {
      method: 'POST',
      credentials: 'same-origin',
      keepalive: true,
    }).catch(() => undefined);
    set({ token: null, user: null });
    window.location.href = '/login';
  },
}));
