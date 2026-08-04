import client from './client';
import type { CurrentMetricsResponse, MetricsHistoryResponse } from '../types/api';

export async function getCurrentMetrics(): Promise<CurrentMetricsResponse> {
  const { data } = await client.get<CurrentMetricsResponse>('/server/metrics/current');
  return data;
}

export async function getMetricsHistory(
  since?: string,
  resolution?: number,
): Promise<MetricsHistoryResponse> {
  const params: Record<string, string | number> = {};
  if (since) params.since = since;
  if (resolution) params.resolution = resolution;
  const { data } = await client.get<MetricsHistoryResponse>('/server/metrics/history', { params });
  return data;
}
