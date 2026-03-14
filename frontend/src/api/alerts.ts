import client from './client';
import type { PaginatedResponse } from '../types/api';
import type { Alert, AlertSummary } from '../types/alerts';

export async function getAlerts(params?: Record<string, unknown>): Promise<PaginatedResponse<Alert>> {
  const { data } = await client.get<PaginatedResponse<Alert>>('/alerts', { params });
  return data;
}

export async function getAlertSummary(): Promise<AlertSummary> {
  const { data } = await client.get<AlertSummary>('/alerts/summary');
  return data;
}

export async function getAlert(id: string): Promise<Alert> {
  const { data } = await client.get<Alert>(`/alerts/${id}`);
  return data;
}

export async function acknowledgeAlert(id: string): Promise<Alert> {
  const { data } = await client.post<Alert>(`/alerts/${id}/acknowledge`);
  return data;
}

export async function resolveAlert(id: string): Promise<Alert> {
  const { data } = await client.post<Alert>(`/alerts/${id}/resolve`);
  return data;
}

export async function bulkAcknowledge(ids: string[]): Promise<void> {
  await client.post('/alerts/bulk/acknowledge', { alert_ids: ids });
}

export async function bulkResolve(ids: string[]): Promise<void> {
  await client.post('/alerts/bulk/resolve', { alert_ids: ids });
}
