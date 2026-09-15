import client from './client';
import type { DeviceMetric, DeviceMetricParams } from '../types/api';

export async function getDeviceMetrics(
  deviceId: string,
  params?: DeviceMetricParams,
): Promise<DeviceMetric[]> {
  const { data } = await client.get<DeviceMetric[]>(`/devices/${deviceId}/metrics`, { params });
  return data;
}
