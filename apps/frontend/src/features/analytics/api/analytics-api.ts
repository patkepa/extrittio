import client from '../../../api/client';
import type { components } from '../../../types/openapi';

export type AnalyticsMetricRequest = components['schemas']['AnalyticsMetricRequest'];
export type AnalyticsMetricCatalogEntry = components['schemas']['AnalyticsMetricCatalogEntry'];
export type AnalyticsSeriesMode = Exclude<
  components['schemas']['AnalyticsSeriesModeName'],
  'latest_ranking'
>;
export type AnalyticsWeighting = components['schemas']['AnalyticsWeightingName'];
export type AnalyticsScopeRequest = components['schemas']['AnalyticsScopeRequest'];
export type AnalyticsQueryRequest = components['schemas']['AnalyticsQueryRequest'];
export type AnalyticsCatalogResponse = components['schemas']['AnalyticsCatalogResponse'];
export type AnalyticsStats = components['schemas']['AnalyticsStatsResponse'];
export type AnalyticsSeries = components['schemas']['AnalyticsSeriesResponse'];
export type AnalyticsDeviceStats = components['schemas']['AnalyticsDeviceStatsResponse'];
export type AnalyticsQueryResponse = components['schemas']['AnalyticsQueryResponse'];

export async function getAnalyticsCatalog(): Promise<AnalyticsCatalogResponse> {
  const { data } = await client.get<AnalyticsCatalogResponse>('/analytics/catalog');
  return data;
}

export async function runAnalyticsQuery(
  request: AnalyticsQueryRequest,
): Promise<AnalyticsQueryResponse> {
  const { data } = await client.post<AnalyticsQueryResponse>('/analytics/query', request);
  return data;
}
