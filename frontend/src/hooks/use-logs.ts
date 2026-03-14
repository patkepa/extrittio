import { useQuery } from '@tanstack/react-query';
import type { LogsParams } from '../types/api';
import { getDeviceLogs } from '../api/logs';
import { queryKeys } from './query-keys';

export function useDeviceLogs(deviceId: string | null, params?: LogsParams) {
  return useQuery({
    queryKey: queryKeys.logs.list(deviceId ?? '', params),
    queryFn: () => getDeviceLogs(deviceId!, params),
    enabled: !!deviceId,
    staleTime: 10_000,
  });
}
