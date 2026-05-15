import { useQuery } from '@tanstack/react-query';
import { getDashboardStats } from '../api/dashboard';
import { queryKeys } from './query-keys';

export function useDashboardStats(options?: { enabled?: boolean }) {
  return useQuery({
    queryKey: queryKeys.dashboard.stats,
    queryFn: getDashboardStats,
    staleTime: 30_000,
    enabled: options?.enabled ?? true,
  });
}
