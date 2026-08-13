import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  createThreadNetwork,
  forceThreadScan,
  getThreadScan,
  getThreadStatus,
  importThreadDataset,
  refreshThreadRuntime,
} from '../api/thread';
import type {
  CreateThreadNetworkRequest,
  ImportThreadDatasetRequest,
  ThreadNetworkDiagnostics,
} from '../types/api';
import { queryKeys } from './query-keys';

export function useThreadStatus() {
  return useQuery({
    queryKey: queryKeys.thread.status,
    queryFn: getThreadStatus,
    staleTime: 10_000,
    refetchInterval: 20_000,
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

export function useForceThreadScan() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationKey: queryKeys.thread.forceScan,
    mutationFn: forceThreadScan,
    onMutate: () => {
      const previousScan = queryClient.getQueryData<ThreadNetworkDiagnostics>(
        queryKeys.thread.scan,
      );
      queryClient.setQueryData<ThreadNetworkDiagnostics>(queryKeys.thread.scan, (scan) =>
        scan ? { ...scan, scanning: true, error: null } : scan,
      );
      return { previousScan };
    },
    onError: (_error, _variables, context) => {
      queryClient.setQueryData(queryKeys.thread.scan, context?.previousScan);
    },
    onSuccess: (scan) => {
      queryClient.setQueryData(queryKeys.thread.scan, scan);
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.thread.scan });
    },
  });
}

export function useThreadRuntimeRefresh() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationKey: queryKeys.thread.refresh,
    mutationFn: refreshThreadRuntime,
    onSuccess: (status) => {
      queryClient.setQueryData(queryKeys.thread.status, status);
    },
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
