import client from './client';
import type { TelemetryRecord, TelemetryParams } from '../types/api';

export async function getDeviceTelemetry(
  deviceId: string,
  params?: TelemetryParams,
): Promise<TelemetryRecord[]> {
  const { data } = await client.get<TelemetryRecord[]>(`/devices/${deviceId}/telemetry`, {
    params,
  });
  return data;
}
