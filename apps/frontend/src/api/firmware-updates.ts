import client from './client';
import type {
  FirmwareUpdate,
  GlobalOtaDeployment,
  CreateFirmwareUpdateRequest,
  FirmwareUpdatesParams,
  NextVersionResponse,
  OtaDeployment,
  OtaDeploymentsParams,
  PaginatedResponse,
  TriggerOtaRequest,
} from '../types/api';

export async function getFirmwareUpdates(
  params?: FirmwareUpdatesParams,
): Promise<FirmwareUpdate[]> {
  const { data } = await client.get<{ data: FirmwareUpdate[] }>('/firmware-updates', {
    params,
  });
  return data.data;
}

export async function createFirmwareUpdate(
  body: CreateFirmwareUpdateRequest,
): Promise<FirmwareUpdate> {
  const { data } = await client.post<FirmwareUpdate>('/firmware-updates', body);
  return data;
}

export interface UploadFirmwareRequest {
  blueprint_revision_id: string;
  version?: string;
  description?: string;
  file: File;
}

export async function uploadFirmwareUpdate(req: UploadFirmwareRequest): Promise<FirmwareUpdate> {
  const formData = new FormData();
  formData.append('blueprint_revision_id', req.blueprint_revision_id);
  if (req.version) formData.append('version', req.version);
  if (req.description) formData.append('description', req.description);
  formData.append('file', req.file);

  const { data } = await client.post<FirmwareUpdate>('/firmware-updates/upload', formData, {
    headers: { 'Content-Type': 'multipart/form-data' },
  });
  return data;
}

export async function deleteFirmwareUpdate(id: number): Promise<void> {
  await client.delete(`/firmware-updates/${id}`);
}

export async function getNextVersion(deviceTypeId: number): Promise<NextVersionResponse> {
  const { data } = await client.get<NextVersionResponse>(
    `/firmware-updates/next-version/${deviceTypeId}`,
  );
  return data;
}

export async function getNextBlueprintVersion(revisionId: string): Promise<NextVersionResponse> {
  const { data } = await client.get<NextVersionResponse>(
    `/firmware-updates/next-version/blueprint/${revisionId}`,
  );
  return data;
}

export async function triggerOta(deviceId: string, body: TriggerOtaRequest): Promise<void> {
  await client.post(`/devices/${deviceId}/ota`, body);
}

export async function getOtaDeployments(deviceId: string): Promise<OtaDeployment[]> {
  const { data } = await client.get<{ data: OtaDeployment[] }>(
    `/devices/${deviceId}/ota-deployments`,
  );
  return data.data;
}

export async function getAllOtaDeployments(
  params?: OtaDeploymentsParams,
): Promise<PaginatedResponse<GlobalOtaDeployment>> {
  const { data } = await client.get<PaginatedResponse<GlobalOtaDeployment>>('/ota-deployments', {
    params,
  });
  return data;
}
