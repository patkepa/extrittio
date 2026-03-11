import client from "./client";
import type { DeviceType, CreateDeviceTypeRequest } from "../types/api";

export async function getDeviceTypes(): Promise<DeviceType[]> {
  const { data } = await client.get<{ data: DeviceType[] }>("/device-types");
  return data.data;
}

export async function createDeviceType(
  body: CreateDeviceTypeRequest
): Promise<DeviceType> {
  const { data } = await client.post<DeviceType>("/device-types", body);
  return data;
}

export async function deleteDeviceType(id: number): Promise<void> {
  await client.delete(`/device-types/${id}`);
}
