import { useQuery, keepPreviousData } from '@tanstack/react-query';
import type { DeviceMetric, DeviceMetricParams } from '../types/api';
import { getDeviceMetrics } from '../api/telemetry';
import { queryKeys } from './query-keys';

const DEFAULT_TELEMETRY_REFETCH_INTERVAL_MS = 10_000;

const METRIC_PAGE_SIZE = 10_000;
const ALL_MAX_METRICS = 100_000;

async function getMetricWindow(
  deviceId: string,
  params?: DeviceMetricParams,
): Promise<DeviceMetric[]> {
  const requestedLimit = params?.limit ?? 1000;
  if (requestedLimit <= METRIC_PAGE_SIZE) {
    return getDeviceMetrics(deviceId, params);
  }

  const metrics: DeviceMetric[] = [];
  let before = params?.before ?? undefined;
  while (metrics.length < requestedLimit) {
    const pageLimit = Math.min(METRIC_PAGE_SIZE, requestedLimit - metrics.length);
    const page = await getDeviceMetrics(deviceId, { ...params, before, limit: pageLimit });
    metrics.push(...page);
    if (page.length < pageLimit) break;
    const nextBefore = page[page.length - 1]?.occurred_at;
    if (!nextBefore || nextBefore === before) break;
    before = nextBefore;
  }
  return metrics;
}

export function useDeviceMetrics(
  deviceId: string | null,
  params?: DeviceMetricParams,
  options?: { refetchInterval?: number | false },
) {
  return useQuery({
    queryKey: queryKeys.metrics.list(deviceId ?? '', params),
    queryFn: () => getMetricWindow(deviceId!, params),
    enabled: !!deviceId,
    staleTime: 10_000,
    refetchInterval: options?.refetchInterval ?? DEFAULT_TELEMETRY_REFETCH_INTERVAL_MS,
    placeholderData: keepPreviousData,
  });
}

export function useAllDeviceMetrics(deviceId: string | null) {
  return useQuery({
    queryKey: queryKeys.metrics.all(deviceId ?? ''),
    queryFn: async () => {
      if (!deviceId) return [];
      const metrics: DeviceMetric[] = [];
      let before: string | undefined;
      while (metrics.length < ALL_MAX_METRICS) {
        const page = await getDeviceMetrics(deviceId, {
          before,
          limit: Math.min(METRIC_PAGE_SIZE, ALL_MAX_METRICS - metrics.length),
        });
        metrics.push(...page);
        if (page.length < METRIC_PAGE_SIZE) break;
        const nextBefore = page[page.length - 1]?.occurred_at;
        if (!nextBefore || nextBefore === before) break;
        before = nextBefore;
      }
      metrics.sort((a, b) => new Date(a.occurred_at).getTime() - new Date(b.occurred_at).getTime());
      return metrics;
    },
    enabled: !!deviceId,
    staleTime: 60_000,
  });
}
