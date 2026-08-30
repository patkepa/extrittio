import { HTMLTable } from '@blueprintjs/core';
import type { AnalyticsQueryResponse } from '../api/analytics-api';
import { formatMetricValue } from '../model/chart-data';

export function DeviceComparisonTable({ result }: { result: AnalyticsQueryResponse }) {
  const unit = result.metric.unit;
  const precision = result.metric.precision ?? 1;

  return (
    <div className="analytics-device-table" tabIndex={0} aria-label="Device comparison table">
      <HTMLTable compact striped interactive={false}>
        <thead>
          <tr>
            <th>Device</th>
            <th>Latest</th>
            <th>Average</th>
            <th>Minimum</th>
            <th>Maximum</th>
            <th>Coverage</th>
          </tr>
        </thead>
        <tbody>
          {result.devices.map((device) => (
            <tr key={device.device_id}>
              <td>
                <span className="analytics-device-dot" aria-hidden="true" />
                {device.device_name}
              </td>
              <td className="mono-data">
                {formatMetricValue(device.stats?.latest, unit, precision)}
              </td>
              <td className="mono-data">
                {formatMetricValue(device.stats?.average, unit, precision)}
              </td>
              <td className="mono-data">
                {formatMetricValue(device.stats?.minimum, unit, precision)}
              </td>
              <td className="mono-data">
                {formatMetricValue(device.stats?.maximum, unit, precision)}
              </td>
              <td className="mono-data analytics-coverage-cell">
                {device.coverage_percent.toFixed(0)}%
              </td>
            </tr>
          ))}
        </tbody>
      </HTMLTable>
    </div>
  );
}
