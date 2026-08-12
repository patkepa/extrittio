import axios, { AxiosHeaders } from "axios";
import type { AxiosError, AxiosInstance } from "axios";

export interface CreateApiClientOptions {
  baseUrl: string;
  getToken?: () => string | null | undefined;
  onUnauthorized?: (error: AxiosError) => void;
  shouldHandleUnauthorized?: (error: AxiosError) => boolean;
}

export function createApiClient({
  baseUrl,
  getToken,
  onUnauthorized,
  shouldHandleUnauthorized,
}: CreateApiClientOptions): AxiosInstance {
  const client = axios.create({
    baseURL: baseUrl,
  });

  client.interceptors.request.use((config) => {
    const token = getToken?.();
    if (token) {
      config.headers = AxiosHeaders.from(config.headers);
      config.headers.set("Authorization", `Bearer ${token}`);
    }
    return config;
  });

  client.interceptors.response.use(
    (response) => response,
    (error: AxiosError) => {
      if (
        error.response?.status === 401 &&
        (shouldHandleUnauthorized?.(error) ?? true)
      ) {
        onUnauthorized?.(error);
      }
      return Promise.reject(error);
    },
  );

  return client;
}
