import { useQuery } from '@tanstack/react-query';
import { getCurrentMetrics, getMetricsHistory } from '../api/server-metrics';
import { queryKeys } from './query-keys';

export function useCurrentMetrics() {
  return useQuery({
    queryKey: queryKeys.serverMetrics.current,
    queryFn: getCurrentMetrics,
    staleTime: 25_000,
    refetchInterval: 30_000,
  });
}

export function useMetricsHistory(since?: string, resolution?: number) {
  return useQuery({
    queryKey: queryKeys.serverMetrics.history({ since, resolution }),
    queryFn: () => getMetricsHistory(since, resolution),
    staleTime: 55_000,
    refetchInterval: 60_000,
  });
}
