import { Callout, Spinner } from '@blueprintjs/core';
import { AreaChart, Area, ResponsiveContainer } from 'recharts';
import { useDeviceTelemetry } from '../../hooks/use-telemetry';

interface TelemetryTabProps {
  deviceId: string;
}

const metrics = [
  { label: 'Temperature', dataKey: 'temperature' as const, color: '#2965CC', unit: '\u00B0C' },
  { label: 'Humidity', dataKey: 'humidity' as const, color: '#0F9960', unit: '%' },
  { label: 'Battery Level', dataKey: 'battery' as const, color: '#D99E0B', unit: '%' },
];

export const TelemetryTab = ({ deviceId }: TelemetryTabProps) => {
  const { data: telemetryRecords = [], isLoading, isError } = useDeviceTelemetry(deviceId, { limit: 50 });

  const chartData = telemetryRecords
    .slice()
    .reverse()
    .map((r) => ({
      temperature: r.temperature,
      humidity: r.humidity,
      battery: r.battery_level,
    }));

  if (isLoading) return <Spinner />;

  if (isError) {
    return (
      <Callout intent="danger" icon="error">
        Failed to load telemetry data. Try refreshing the page.
      </Callout>
    );
  }

  if (chartData.length === 0) {
    return (
      <Callout icon="info-sign" intent="primary">
        No telemetry data available for this device.
      </Callout>
    );
  }

  return (
    <div className="telemetry-tab">
      {metrics.map((metric) => {
        const latestValue = chartData[chartData.length - 1]?.[metric.dataKey];
        return (
          <div key={metric.label} className="telemetry-chart">
            <div className="telemetry-header">
              <span className="section-label">{metric.label}</span>
              <span className="mono-data" style={{ fontSize: 14, color: metric.color }}>
                {latestValue != null ? `${latestValue}${metric.unit}` : '—'}
              </span>
            </div>
            <ResponsiveContainer width="100%" height={60}>
              <AreaChart data={chartData}>
                <defs>
                  <linearGradient id={`tel-${metric.label.replace(/\s/g, '')}`} x1="0" y1="0" x2="0" y2="1">
                    <stop offset="0%" stopColor={metric.color} stopOpacity={0.3} />
                    <stop offset="100%" stopColor={metric.color} stopOpacity={0} />
                  </linearGradient>
                </defs>
                <Area
                  type="monotone"
                  dataKey={metric.dataKey}
                  stroke={metric.color}
                  strokeWidth={1.5}
                  fill={`url(#tel-${metric.label.replace(/\s/g, '')})`}
                  dot={false}
                  isAnimationActive={false}
                  connectNulls
                />
              </AreaChart>
            </ResponsiveContainer>
          </div>
        );
      })}
    </div>
  );
};
