import client from './client';
import type { LogRecord, LogsParams } from '../types/api';

export async function getDeviceLogs(deviceId: string, params?: LogsParams): Promise<LogRecord[]> {
  const { data } = await client.get<LogRecord[]>(`/devices/${deviceId}/logs`, { params });
  return data;
}
