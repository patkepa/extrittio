import { createApiClient } from '@extrittio/data-client';
import { useAuthStore } from '../stores/auth-store';

export const EXTRITTIO_API_BASE_URL = '/api/v1';

const client = createApiClient({
  baseUrl: EXTRITTIO_API_BASE_URL,
  getToken: () => useAuthStore.getState().token,
  shouldHandleUnauthorized: () => window.location.pathname !== '/login',
  onUnauthorized: () => {
    useAuthStore.getState().logout();
  },
});

export default client;
