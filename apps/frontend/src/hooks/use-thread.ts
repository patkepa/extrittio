import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  createThreadNetwork,
  getThreadScan,
  getThreadStatus,
  importThreadDataset,
} from '../api/thread';
import type { CreateThreadNetworkRequest, ImportThreadDatasetRequest } from '../types/api';
import { queryKeys } from './query-keys';

export function useThreadStatus() {
  return useQuery({
    queryKey: queryKeys.thread.status,
    queryFn: getThreadStatus,
    staleTime: 5_000,
    refetchInterval: (query) => (query.state.data?.connected ? 20_000 : 5_000),
  });
}

export function useThreadScan() {
  return useQuery({
    queryKey: queryKeys.thread.scan,
    queryFn: getThreadScan,
    refetchOnMount: 'always',
    refetchInterval: (query) => (query.state.data?.scanning ? 1_000 : 5_000),
  });
}

export function useCreateThreadNetwork() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: CreateThreadNetworkRequest) => createThreadNetwork(body),
    onSuccess: (status) => {
      queryClient.setQueryData(queryKeys.thread.status, status);
      void queryClient.invalidateQueries({ queryKey: queryKeys.thread.scan });
    },
  });
}

export function useImportThreadDataset() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: ImportThreadDatasetRequest) => importThreadDataset(body),
    onSuccess: (status) => {
      queryClient.setQueryData(queryKeys.thread.status, status);
      void queryClient.invalidateQueries({ queryKey: queryKeys.thread.scan });
    },
  });
}
