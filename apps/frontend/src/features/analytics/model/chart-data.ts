import type { AlignedData } from 'uplot';
import type { AnalyticsSeries } from '../api/analytics-api';

export function alignAnalyticsSeries(series: AnalyticsSeries[]): AlignedData {
  const timestamps = Array.from(
    new Set(series.flatMap((item) => item.points.map((point) => point.timestamp_ms))),
  ).sort((left, right) => left - right);
  const timestampIndex = new Map(timestamps.map((timestamp, index) => [timestamp, index]));
  const values = series.map(() => Array<number | null>(timestamps.length).fill(null));

  series.forEach((item, seriesIndex) => {
    item.points.forEach((point) => {
      const index = timestampIndex.get(point.timestamp_ms);
      if (index !== undefined) values[seriesIndex]![index] = point.value;
    });
  });

  return [timestamps.map((timestamp) => timestamp / 1000), ...values] as AlignedData;
}

export function formatBucket(seconds: number): string {
  if (seconds < 60) return `${seconds} sec`;
  if (seconds < 3_600) return `${seconds / 60} min`;
  if (seconds < 86_400) return `${seconds / 3_600} hr`;
  return `${seconds / 86_400} day`;
}

export function formatMetricValue(
  value: number | null | undefined,
  unit: string | null | undefined,
  precision = 1,
): string {
  return value == null ? '—' : `${value.toFixed(precision)}${unit ?? ''}`;
}
