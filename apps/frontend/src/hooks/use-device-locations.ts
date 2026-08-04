import { useQuery } from '@tanstack/react-query';
import client from '../api/client';
import { queryKeys } from './query-keys';
import type { LocationPoint } from '../types/api';

export function useLatestLocation(deviceId: string | null) {
  return useQuery({
    queryKey: queryKeys.locations.latest(deviceId!),
    queryFn: async () => {
      const { data } = await client.get<LocationPoint | null>(
        `/devices/${deviceId}/location/latest`,
      );
      return data;
    },
    enabled: !!deviceId,
    staleTime: 10_000,
  });
}
