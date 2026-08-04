import client from './client';
import type { Zone, CreateZoneRequest, UpdateZoneRequest } from '../types/zones';

export async function getZones(): Promise<Zone[]> {
  const { data } = await client.get<Zone[]>('/zones');
  return data;
}

export async function getZone(id: string): Promise<Zone> {
  const { data } = await client.get<Zone>(`/zones/${id}`);
  return data;
}

export async function createZone(body: CreateZoneRequest): Promise<Zone> {
  const { data } = await client.post<Zone>('/zones', body);
  return data;
}

export async function updateZone(id: string, body: UpdateZoneRequest): Promise<Zone> {
  const { data } = await client.put<Zone>(`/zones/${id}`, body);
  return data;
}

export async function deleteZone(id: string): Promise<void> {
  await client.delete(`/zones/${id}`);
}
