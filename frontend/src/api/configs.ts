import client from "./client";
import type { DeviceConfigResponse } from "../types/api";

export async function getDeviceConfig(
  deviceId: string
): Promise<DeviceConfigResponse> {
  const { data } = await client.get<DeviceConfigResponse>(
    `/devices/${deviceId}/config`
  );
  return data;
}

export async function updateDeviceConfig(
  deviceId: string,
  config: Record<string, unknown>
): Promise<DeviceConfigResponse> {
  const { data } = await client.put<DeviceConfigResponse>(
    `/devices/${deviceId}/config`,
    config
  );
  return data;
}
