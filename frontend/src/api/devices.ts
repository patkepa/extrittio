import client from "./client";
import type {
  Device,
  CreateDeviceRequest,
  UpdateDeviceRequest,
  ListDevicesParams,
} from "../types/api";

export async function getDevices(params?: ListDevicesParams): Promise<Device[]> {
  const { data } = await client.get<{ data: Device[] }>("/devices", { params });
  return data.data;
}

export async function getDevice(id: string): Promise<Device> {
  const { data } = await client.get<Device>(`/devices/${id}`);
  return data;
}

export async function createDevice(body: CreateDeviceRequest): Promise<Device> {
  const { data } = await client.post<Device>("/devices", body);
  return data;
}

export async function updateDevice(
  id: string,
  body: UpdateDeviceRequest
): Promise<Device> {
  const { data } = await client.put<Device>(`/devices/${id}`, body);
  return data;
}

export async function deleteDevice(id: string): Promise<void> {
  await client.delete(`/devices/${id}`);
}

export async function restartDevice(id: string): Promise<void> {
  await client.post(`/devices/${id}/restart`);
}
