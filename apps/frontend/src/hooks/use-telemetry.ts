import { useQuery, keepPreviousData } from '@tanstack/react-query';
import type {
  DeviceMetric,
  DeviceMetricParams,
  TelemetryParams,
  TelemetryRecord,
} from '../types/api';
import { getDeviceMetrics, getDeviceTelemetry } from '../api/telemetry';
import { queryKeys } from './query-keys';

const API_PAGE_SIZE = 1000;
const DEFAULT_TELEMETRY_REFETCH_INTERVAL_MS = 10_000;

async function getTelemetryWindow(
  deviceId: string,
  params?: TelemetryParams,
): Promise<TelemetryRecord[]> {
  const requestedLimit = params?.limit ?? 50;
  if (requestedLimit <= API_PAGE_SIZE) {
    return getDeviceTelemetry(deviceId, params);
  }

  const records: TelemetryRecord[] = [];
  let before = params?.before;
  while (records.length < requestedLimit) {
    const pageLimit = Math.min(API_PAGE_SIZE, requestedLimit - records.length);
    const page = await getDeviceTelemetry(deviceId, { ...params, before, limit: pageLimit });
    records.push(...page);
    if (page.length < pageLimit) break;

    const nextBefore = page[page.length - 1]?.received_at;
    if (!nextBefore || nextBefore === before) break;
    before = nextBefore;
  }
  return records;
}

export function useDeviceTelemetry(
  deviceId: string | null,
  params?: TelemetryParams,
  options?: { refetchInterval?: number | false },
) {
  return useQuery({
    queryKey: queryKeys.telemetry.list(deviceId ?? '', params),
    queryFn: () => getTelemetryWindow(deviceId!, params),
    enabled: !!deviceId,
    staleTime: 10_000,
    refetchInterval: options?.refetchInterval ?? DEFAULT_TELEMETRY_REFETCH_INTERVAL_MS,
    placeholderData: keepPreviousData,
  });
}

const ALL_PAGE_SIZE = 1000;
const ALL_MAX_RECORDS = 50_000;

export function useAllDeviceTelemetry(deviceId: string | null) {
  return useQuery({
    queryKey: queryKeys.telemetry.all(deviceId ?? ''),
    queryFn: async () => {
      if (!deviceId) return [];
      const allRecords: TelemetryRecord[] = [];
      let cursor: string | undefined;

      while (allRecords.length < ALL_MAX_RECORDS) {
        const params: TelemetryParams = { limit: ALL_PAGE_SIZE };
        if (cursor) {
          params.before = cursor;
        }
        const batch = await getDeviceTelemetry(deviceId, params);
        allRecords.push(...batch);

        if (batch.length < ALL_PAGE_SIZE) break;
        // Use the oldest record's timestamp as cursor for next page
        const lastRecord = batch[batch.length - 1];
        const prevCursor = cursor;
        cursor = lastRecord?.received_at;
        // Guard against infinite loop when timestamps don't advance
        if (cursor === prevCursor) break;
      }

      // Sort chronologically (oldest first) for consistent chart rendering.
      // Backend returns DESC per page; explicit sort guarantees correct order
      // regardless of page boundaries or duplicate timestamps.
      allRecords.sort(
        (a, b) => new Date(a.received_at).getTime() - new Date(b.received_at).getTime(),
      );

      return allRecords;
    },
    enabled: !!deviceId,
    staleTime: 60_000,
  });
}

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
