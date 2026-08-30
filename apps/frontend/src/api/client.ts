import axios from 'axios';
import type { AxiosError } from 'axios';
import { useAuthStore } from '../stores/auth-store';

export const EXTRITTIO_API_BASE_URL = '/api/v1';

// The browser authenticates with the HttpOnly SameSite session cookie.
const client = axios.create({
  baseURL: EXTRITTIO_API_BASE_URL,
});

client.interceptors.response.use(
  (response) => response,
  (error: AxiosError) => {
    if (error.response?.status === 401 && window.location.pathname !== '/login') {
      useAuthStore.getState().logout();
    }
    return Promise.reject(error);
  },
);

export default client;
