import client from './client';
import type { DeviceShadow } from '../types/api';

export async function getDeviceShadow(deviceId: string): Promise<DeviceShadow> {
  const { data } = await client.get<DeviceShadow>(`/devices/${deviceId}/shadow`);
  return data;
}

export async function updateDesiredState(
  deviceId: string,
  state: Record<string, unknown>,
): Promise<DeviceShadow> {
  const { data } = await client.put<DeviceShadow>(`/devices/${deviceId}/shadow/desired`, state);
  return data;
}

export async function updateReportedState(
  deviceId: string,
  state: Record<string, unknown>,
): Promise<DeviceShadow> {
  const { data } = await client.put<DeviceShadow>(`/devices/${deviceId}/shadow/reported`, state);
  return data;
}

export async function deleteDeviceShadow(deviceId: string): Promise<void> {
  await client.delete(`/devices/${deviceId}/shadow`);
}
