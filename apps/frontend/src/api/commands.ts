import client from './client';
import type { CommandRecord, SendCommandRequest, CommandsParams } from '../types/api';

export async function sendCommand(
  deviceId: string,
  body: SendCommandRequest,
): Promise<CommandRecord> {
  const { data } = await client.post<CommandRecord>(`/devices/${deviceId}/commands`, body);
  return data;
}

export async function getCommandHistory(
  deviceId: string,
  params?: CommandsParams,
): Promise<CommandRecord[]> {
  const { data } = await client.get<CommandRecord[]>(`/devices/${deviceId}/commands`, { params });
  return data;
}
