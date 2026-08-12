import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { createThreadNetwork, getThreadStatus, importThreadDataset } from '../api/thread';
import type { CreateThreadNetworkRequest, ImportThreadDatasetRequest } from '../types/api';
import { queryKeys } from './query-keys';

export function useThreadStatus() {
  return useQuery({
    queryKey: queryKeys.thread.status,
    queryFn: getThreadStatus,
    staleTime: 10_000,
    refetchInterval: 20_000,
  });
}

export function useCreateThreadNetwork() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: CreateThreadNetworkRequest) => createThreadNetwork(body),
    onSuccess: (status) => {
      queryClient.setQueryData(queryKeys.thread.status, status);
    },
  });
}

export function useImportThreadDataset() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: ImportThreadDatasetRequest) => importThreadDataset(body),
    onSuccess: (status) => {
      queryClient.setQueryData(queryKeys.thread.status, status);
    },
  });
}
