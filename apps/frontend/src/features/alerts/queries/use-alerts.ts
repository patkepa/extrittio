import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  getAlerts,
  getAlertSummary,
  acknowledgeAlert,
  resolveAlert,
  reactivateAlert,
  bulkAcknowledge,
  bulkResolve,
  bulkReactivate,
} from '../api/alerts';
import { queryKeys } from '../../../hooks/query-keys';

interface AlertQueryOptions {
  enabled?: boolean;
  refetchInterval?: number | false;
}

export function useAlerts(params?: Record<string, unknown>, options?: AlertQueryOptions) {
  return useQuery({
    queryKey: queryKeys.alerts.list(params),
    queryFn: () => getAlerts(params),
    staleTime: 10_000,
    refetchInterval: options?.refetchInterval ?? 10_000,
    enabled: options?.enabled ?? true,
  });
}

export function useAlertSummary(options?: AlertQueryOptions) {
  return useQuery({
    queryKey: queryKeys.alerts.summary,
    queryFn: getAlertSummary,
    staleTime: 15_000,
    refetchInterval: 30_000,
    enabled: options?.enabled ?? true,
  });
}

export function useAcknowledgeAlert() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => acknowledgeAlert(id),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.alerts.all });
      void queryClient.invalidateQueries({ queryKey: queryKeys.alerts.summary });
    },
  });
}

export function useResolveAlert() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => resolveAlert(id),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.alerts.all });
      void queryClient.invalidateQueries({ queryKey: queryKeys.alerts.summary });
    },
  });
}

export function useBulkAcknowledge() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (ids: string[]) => bulkAcknowledge(ids),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.alerts.all });
      void queryClient.invalidateQueries({ queryKey: queryKeys.alerts.summary });
    },
  });
}

export function useBulkResolve() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (ids: string[]) => bulkResolve(ids),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.alerts.all });
      void queryClient.invalidateQueries({ queryKey: queryKeys.alerts.summary });
    },
  });
}

export function useReactivateAlert() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => reactivateAlert(id),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.alerts.all });
      void queryClient.invalidateQueries({ queryKey: queryKeys.alerts.summary });
    },
  });
}

export function useBulkReactivate() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (ids: string[]) => bulkReactivate(ids),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.alerts.all });
      void queryClient.invalidateQueries({ queryKey: queryKeys.alerts.summary });
    },
  });
}
