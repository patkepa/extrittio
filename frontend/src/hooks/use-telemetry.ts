import { useQuery, keepPreviousData } from "@tanstack/react-query";
import type { TelemetryParams, TelemetryRecord } from "../types/api";
import { getDeviceTelemetry } from "../api/telemetry";
import { queryKeys } from "./query-keys";

export function useDeviceTelemetry(
  deviceId: string | null,
  params?: TelemetryParams
) {
  return useQuery({
    queryKey: queryKeys.telemetry.list(deviceId ?? "", params),
    queryFn: () => getDeviceTelemetry(deviceId!, params),
    enabled: !!deviceId,
    staleTime: 10_000,
    placeholderData: keepPreviousData,
  });
}

const ALL_PAGE_SIZE = 1000;
const ALL_MAX_RECORDS = 50_000;

export function useAllDeviceTelemetry(deviceId: string | null) {
  return useQuery({
    queryKey: queryKeys.telemetry.all(deviceId ?? ""),
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
        (a, b) => new Date(a.received_at).getTime() - new Date(b.received_at).getTime()
      );

      return allRecords;
    },
    enabled: !!deviceId,
    staleTime: 60_000,
  });
}
