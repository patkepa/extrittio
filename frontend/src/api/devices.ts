import client from "./client";
import type {
  Device,
  CreateDeviceRequest,
  UpdateDeviceRequest,
  ListDevicesParams,
  BulkTargeting,
  BulkFleetRequest,
  BulkOtaRequest,
  BulkAffectedResponse,
  BulkResultResponse,
  PaginatedResponse,
} from "../types/api";

export async function getDevices(params?: ListDevicesParams): Promise<PaginatedResponse<Device>> {
  const { data } = await client.get<PaginatedResponse<Device>>("/devices", { params });
  return data;
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

export async function bulkChangeFleet(body: BulkFleetRequest): Promise<BulkAffectedResponse> {
  const { data } = await client.post<BulkAffectedResponse>("/devices/bulk/fleet", body);
  return data;
}

export async function bulkDeleteDevices(body: BulkTargeting): Promise<BulkAffectedResponse> {
  const { data } = await client.post<BulkAffectedResponse>("/devices/bulk/delete", body);
  return data;
}

export async function bulkRestartDevices(body: BulkTargeting): Promise<BulkResultResponse> {
  const { data } = await client.post<BulkResultResponse>("/devices/bulk/restart", body);
  return data;
}

export async function bulkTriggerOta(body: BulkOtaRequest): Promise<BulkResultResponse> {
  const { data } = await client.post<BulkResultResponse>("/devices/bulk/ota", body);
  return data;
}
