import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  getAlerts,
  getAlertSummary,
  acknowledgeAlert,
  resolveAlert,
  bulkAcknowledge,
  bulkResolve,
} from '../api/alerts';
import { queryKeys } from './query-keys';

export function useAlerts(params?: Record<string, unknown>) {
  return useQuery({
    queryKey: queryKeys.alerts.list(params),
    queryFn: () => getAlerts(params),
    staleTime: 10_000,
  });
}

export function useAlertSummary() {
  return useQuery({
    queryKey: queryKeys.alerts.summary,
    queryFn: getAlertSummary,
    staleTime: 15_000,
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
