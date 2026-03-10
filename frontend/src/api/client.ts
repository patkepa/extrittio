import axios from "axios";
import { useAuthStore } from "../stores/auth-store";

const client = axios.create({
  baseURL: "/api",
});

// Request interceptor: attach JWT token
client.interceptors.request.use((config) => {
  const token = localStorage.getItem("token");
  if (token) {
    config.headers.Authorization = `Bearer ${token}`;
  }
  return config;
});

// Response interceptor: redirect on 401
client.interceptors.response.use(
  (response) => response,
  (error) => {
    if (error.response?.status === 401 && window.location.pathname !== "/login") {
      useAuthStore.getState().logout();
    }
    return Promise.reject(error);
  }
);

export default client;
