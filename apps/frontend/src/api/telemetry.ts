import client from './client';
import type {
  DeviceMetric,
  DeviceMetricParams,
  TelemetryRecord,
  TelemetryParams,
} from '../types/api';

export async function getDeviceTelemetry(
  deviceId: string,
  params?: TelemetryParams,
): Promise<TelemetryRecord[]> {
  const { data } = await client.get<TelemetryRecord[]>(`/devices/${deviceId}/telemetry`, {
    params,
  });
  return data;
}

export async function getDeviceMetrics(
  deviceId: string,
  params?: DeviceMetricParams,
): Promise<DeviceMetric[]> {
  const { data } = await client.get<DeviceMetric[]>(`/devices/${deviceId}/metrics`, { params });
  return data;
}
