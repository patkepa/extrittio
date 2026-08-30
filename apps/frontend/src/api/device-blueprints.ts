import client from './client';
import type {
  DeviceBlueprint,
  DeviceBlueprintDocument,
  DeviceBlueprintDraft,
  DeviceBlueprintList,
  DeviceBlueprintRevision,
  DeviceBlueprintValidation,
} from '../types/api';

export async function getDeviceBlueprints(): Promise<DeviceBlueprint[]> {
  const { data } = await client.get<DeviceBlueprintList>('/device-blueprints', {
    params: { limit: 500, offset: 0 },
  });
  return data.data;
}

export async function getLatestDeviceBlueprintRevision(
  blueprintId: string,
): Promise<DeviceBlueprintRevision> {
  const { data } = await client.get<DeviceBlueprintRevision>(
    `/device-blueprints/${blueprintId}/revisions/latest`,
  );
  return data;
}

export async function getDeviceBlueprintDraft(blueprintId: string): Promise<DeviceBlueprintDraft> {
  const { data } = await client.get<DeviceBlueprintDraft>(
    `/device-blueprints/${blueprintId}/draft`,
  );
  return data;
}

export async function createDeviceBlueprint(
  document: DeviceBlueprintDocument,
): Promise<DeviceBlueprint> {
  const { data } = await client.post<DeviceBlueprint>('/device-blueprints', { document });
  return data;
}

export async function replaceDeviceBlueprintDraft(
  blueprintId: string,
  document: DeviceBlueprintDocument,
): Promise<DeviceBlueprintDraft> {
  const { data } = await client.put<DeviceBlueprintDraft>(
    `/device-blueprints/${blueprintId}/draft`,
    { document },
  );
  return data;
}

export async function validateDeviceBlueprintDraft(
  blueprintId: string,
): Promise<DeviceBlueprintValidation> {
  const { data } = await client.post<DeviceBlueprintValidation>(
    `/device-blueprints/${blueprintId}/draft/validate`,
  );
  return data;
}

export async function publishDeviceBlueprintDraft(
  blueprintId: string,
): Promise<DeviceBlueprintRevision> {
  const { data } = await client.post<DeviceBlueprintRevision>(
    `/device-blueprints/${blueprintId}/draft/publish`,
  );
  return data;
}
