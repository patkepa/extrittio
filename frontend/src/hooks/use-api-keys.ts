import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createApiKey, deleteApiKey, getApiKeys } from "../api/api-keys";
import type { CreateApiKeyRequest } from "../types/api";
import { queryKeys } from "./query-keys";

export function useApiKeys() {
  return useQuery({
    queryKey: queryKeys.apiKeys.all,
    queryFn: getApiKeys,
    staleTime: 30_000,
  });
}

export function useCreateApiKey() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: CreateApiKeyRequest) => createApiKey(body),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.apiKeys.all });
    },
  });
}

export function useDeleteApiKey() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => deleteApiKey(id),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.apiKeys.all });
    },
  });
}
