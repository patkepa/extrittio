import { useQuery } from "@tanstack/react-query";
import type { TelemetryParams } from "../types/api";
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
  });
}
