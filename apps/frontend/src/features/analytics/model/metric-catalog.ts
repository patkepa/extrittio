import type { AnalyticsMetricCatalogEntry } from '../api/analytics-api';

export interface AnalyticsMetricGroup {
  blueprintId: string;
  blueprintName: string;
  metrics: AnalyticsMetricCatalogEntry[];
}

export function analyticsMetricId(
  metric: Pick<AnalyticsMetricCatalogEntry, 'blueprint_id' | 'stream_key' | 'field_path'>,
): string {
  return JSON.stringify([metric.blueprint_id, metric.stream_key, metric.field_path]);
}

export function groupAnalyticsMetrics(
  metrics: AnalyticsMetricCatalogEntry[],
): AnalyticsMetricGroup[] {
  const groups = new Map<string, AnalyticsMetricGroup>();
  for (const metric of metrics) {
    const existing = groups.get(metric.blueprint_id);
    if (existing) {
      existing.metrics.push(metric);
    } else {
      groups.set(metric.blueprint_id, {
        blueprintId: metric.blueprint_id,
        blueprintName: metric.blueprint_name,
        metrics: [metric],
      });
    }
  }
  return Array.from(groups.values());
}
