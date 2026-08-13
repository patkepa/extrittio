import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  createThreadNetwork,
  getThreadStatus,
  importThreadDataset,
  refreshThreadRuntime,
  scanThreadMesh,
  scanThreadNetworks,
} from '../api/thread';
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

export function useThreadNetworkScan() {
  return useMutation({
    mutationKey: queryKeys.thread.scan,
    mutationFn: scanThreadNetworks,
  });
}

export function useThreadMeshScan() {
  return useMutation({
    mutationKey: queryKeys.thread.meshScan,
    mutationFn: scanThreadMesh,
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
