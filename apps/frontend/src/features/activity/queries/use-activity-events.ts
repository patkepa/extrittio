import { useQuery } from '@tanstack/react-query';
import { queryKeys } from '../../../hooks/query-keys';
import type { ActivityEventsParams } from '../../../types/api';
import { getActivityEvents } from '../api/activity';

interface ActivityQueryOptions {
  refetchInterval?: number | false;
}

export function useActivityEvents(params?: ActivityEventsParams, options?: ActivityQueryOptions) {
  return useQuery({
    queryKey: queryKeys.activity.list(params),
    queryFn: () => getActivityEvents(params),
    staleTime: 5_000,
    refetchInterval: options?.refetchInterval ?? false,
    placeholderData: (previous) => previous,
  });
}
