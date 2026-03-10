import client from "./client";
import type {
  FirmwareUpdate,
  CreateFirmwareUpdateRequest,
  FirmwareUpdatesParams,
  NextVersionResponse,
  OtaDeployment,
  TriggerOtaRequest,
} from "../types/api";

export async function getFirmwareUpdates(
  params?: FirmwareUpdatesParams
): Promise<FirmwareUpdate[]> {
  const { data } = await client.get<FirmwareUpdate[]>("/firmware-updates", {
    params,
  });
  return data;
}

export async function createFirmwareUpdate(
  body: CreateFirmwareUpdateRequest
): Promise<FirmwareUpdate> {
  const { data } = await client.post<FirmwareUpdate>("/firmware-updates", body);
  return data;
}

export async function deleteFirmwareUpdate(id: number): Promise<void> {
  await client.delete(`/firmware-updates/${id}`);
}

export async function getNextVersion(
  deviceTypeId: number
): Promise<NextVersionResponse> {
  const { data } = await client.get<NextVersionResponse>(
    `/firmware-updates/next-version/${deviceTypeId}`
  );
  return data;
}

export async function triggerOta(
  deviceId: string,
  body: TriggerOtaRequest
): Promise<void> {
  await client.post(`/devices/${deviceId}/ota`, body);
}

export async function getOtaDeployments(
  deviceId: string
): Promise<OtaDeployment[]> {
  const { data } = await client.get<OtaDeployment[]>(
    `/devices/${deviceId}/ota-deployments`
  );
  return data;
}
