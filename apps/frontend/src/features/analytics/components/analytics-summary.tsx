import { formatBucket, formatMetricValue } from '../model/chart-data';
import type { AnalyticsQueryResponse } from '../api/analytics-api';

export function AnalyticsSummary({ result }: { result: AnalyticsQueryResponse }) {
  const coverageCount = result.devices.filter((device) => device.stats != null).length;
  const unit = result.metric.unit;
  const precision = result.metric.precision ?? 1;

  return (
    <div className="analytics-summary" aria-label="Analytics summary">
      <div className="analytics-summary__item">
        <span>Average</span>
        <strong>{formatMetricValue(result.stats?.average, unit, precision)}</strong>
      </div>
      <div className="analytics-summary__item">
        <span>Range</span>
        <strong>
          {result.stats
            ? `${result.stats.minimum.toFixed(precision)}–${result.stats.maximum.toFixed(precision)}${unit ?? ''}`
            : '—'}
        </strong>
      </div>
      <div className="analytics-summary__item">
        <span>Reporting devices</span>
        <strong className="analytics-summary__coverage">
          {coverageCount}/{result.scope.compatible_devices}
        </strong>
      </div>
      <div className="analytics-summary__item">
        <span>Resolution</span>
        <strong>{formatBucket(result.effective.bucket_seconds)}</strong>
      </div>
    </div>
  );
}
