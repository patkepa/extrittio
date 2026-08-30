import { keepPreviousData, skipToken, useQuery } from '@tanstack/react-query';
import { queryKeys } from '../../../hooks/query-keys';
import {
  getAnalyticsCatalog,
  runAnalyticsQuery,
  type AnalyticsQueryRequest,
} from '../api/analytics-api';

export function useAnalyticsCatalog() {
  return useQuery({
    queryKey: queryKeys.analytics.catalog,
    queryFn: getAnalyticsCatalog,
    staleTime: 5 * 60_000,
  });
}

export function useAnalyticsQuery(request: AnalyticsQueryRequest | null) {
  return useQuery({
    queryKey: queryKeys.analytics.query(request),
    queryFn: request == null ? skipToken : () => runAnalyticsQuery(request),
    enabled: request != null,
    placeholderData: keepPreviousData,
    staleTime: 30_000,
    retry: false,
  });
}
