import { Callout, Spinner } from '@blueprintjs/core';
import { AreaChart, Area, ResponsiveContainer, XAxis, YAxis, CartesianGrid, Tooltip } from 'recharts';
import { useDeviceTelemetry } from '../../hooks/use-telemetry';
import type { TelemetryRecord } from '../../types/api';

interface TelemetryTabProps {
  deviceId: string;
  deviceTypeName: string;
}

// ---------------------------------------------------------------------------
// Per-device-type metric profiles
// ---------------------------------------------------------------------------

interface MetricDef {
  label: string;
  /** Key into the flattened telemetry row (top-level field or custom_json key) */
  key: string;
  color: string;
  unit: string;
}

interface InfoField {
  label: string;
  key: string;
}

interface DeviceProfile {
  currentValues: MetricDef[];
  charts: MetricDef[];
  tableColumns: MetricDef[];
  /** Non-numeric info fields from custom_json to show below current values */
  infoFields?: InfoField[];
}

const DEFAULT_PROFILE: DeviceProfile = {
  currentValues: [
    { label: 'Temperature', key: 'temperature', color: '#2965CC', unit: '°C' },
    { label: 'Humidity', key: 'humidity', color: '#0F9960', unit: '%' },
    { label: 'Battery Level', key: 'battery_level', color: '#D99E0B', unit: '%' },
  ],
  charts: [
    { label: 'Temperature', key: 'temperature', color: '#2965CC', unit: '°C' },
    { label: 'Humidity', key: 'humidity', color: '#0F9960', unit: '%' },
    { label: 'Battery Level', key: 'battery_level', color: '#D99E0B', unit: '%' },
  ],
  tableColumns: [
    { label: 'Temperature', key: 'temperature', color: '#2965CC', unit: '°C' },
    { label: 'Humidity', key: 'humidity', color: '#0F9960', unit: '%' },
    { label: 'Battery', key: 'battery_level', color: '#D99E0B', unit: '%' },
  ],
};

const MAC_DEVICE_PROFILE: DeviceProfile = {
  currentValues: [
    { label: 'CPU Usage', key: 'cpu_usage_percent', color: '#2965CC', unit: '%' },
    { label: 'Memory Usage', key: 'memory_usage_percent', color: '#9BBF30', unit: '%' },
    { label: 'Battery Level', key: 'battery_level', color: '#D99E0B', unit: '%' },
    { label: 'Load (1m)', key: 'load_1m', color: '#D13913', unit: '' },
    { label: 'Load (5m)', key: 'load_5m', color: '#D13913', unit: '' },
    { label: 'Load (15m)', key: 'load_15m', color: '#D13913', unit: '' },
    { label: 'Battery Health', key: 'battery_health_percent', color: '#0F9960', unit: '%' },
    { label: 'Memory Total', key: 'memory_total_gb', color: '#9BBF30', unit: 'GB' },
  ],
  charts: [
    { label: 'CPU Usage', key: 'cpu_usage_percent', color: '#2965CC', unit: '%' },
    { label: 'Memory Usage', key: 'memory_usage_percent', color: '#9BBF30', unit: '%' },
    { label: 'Battery Level', key: 'battery_level', color: '#D99E0B', unit: '%' },
  ],
  tableColumns: [
    { label: 'CPU %', key: 'cpu_usage_percent', color: '#2965CC', unit: '%' },
    { label: 'Memory %', key: 'memory_usage_percent', color: '#9BBF30', unit: '%' },
    { label: 'Battery', key: 'battery_level', color: '#D99E0B', unit: '%' },
    { label: 'Load 1m', key: 'load_1m', color: '#D13913', unit: '' },
  ],
  infoFields: [
    { label: 'Hostname', key: 'hostname' },
    { label: 'OS', key: 'os_version' },
    { label: 'Chip', key: 'chip_model' },
    { label: 'Battery State', key: 'battery_state' },
    { label: 'Battery Cycles', key: 'battery_cycles' },
  ],
};

const PROFILES: Record<string, DeviceProfile> = {
  'mac-device': MAC_DEVICE_PROFILE,
};

function getProfile(deviceTypeName: string): DeviceProfile {
  return PROFILES[deviceTypeName] ?? DEFAULT_PROFILE;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatTimestamp(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleString(undefined, {
    month: 'short', day: 'numeric',
    hour: '2-digit', minute: '2-digit', second: '2-digit',
  });
}

function formatValue(v: number | string | null | undefined): string {
  if (v == null || v === '') return '—';
  const n = typeof v === 'string' ? parseFloat(v) : v;
  if (isNaN(n)) return String(v);
  return Number.isInteger(n) ? String(n) : n.toFixed(2);
}

/** Parse custom_json into a flat object, returns empty object on failure. */
function parseCustomJson(raw: string | null | undefined): Record<string, string> {
  if (!raw) return {};
  try {
    const parsed = JSON.parse(raw);
    if (typeof parsed === 'object' && parsed !== null && !Array.isArray(parsed)) {
      return parsed as Record<string, string>;
    }
    return {};
  } catch {
    return {};
  }
}

/** Flatten a telemetry record: top-level numeric fields + custom_json keys merged. */
function flattenRecord(r: TelemetryRecord): Record<string, number | string | null> {
  const custom = parseCustomJson(r.custom_json);
  const flat: Record<string, number | string | null> = {
    received_at: r.received_at,
    temperature: r.temperature ?? null,
    humidity: r.humidity ?? null,
    battery_level: r.battery_level ?? null,
  };
  for (const [k, v] of Object.entries(custom)) {
    const n = parseFloat(v);
    flat[k] = isNaN(n) ? v : n;
  }
  return flat;
}

/** Short time label for X-axis ticks */
function formatAxisTime(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });
}

/** Whether a metric should use a 0-100 domain (percentage metrics) */
function isPercentMetric(unit: string): boolean {
  return unit === '%';
}

/** Custom tooltip */
function ChartTooltip({ active, payload, metric }: {
  active?: boolean;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  payload?: Array<{ value: number; payload: any }>;
  label?: string;
  metric: MetricDef;
}) {
  const entry = payload?.[0];
  if (!active || !entry) return null;
  const time = entry.payload?.received_at;
  return (
    <div className="telemetry-tooltip">
      {time && <div className="telemetry-tooltip-time">{formatTimestamp(String(time))}</div>}
      <div className="telemetry-tooltip-value" style={{ color: metric.color }}>
        {formatValue(entry.value)} {metric.unit}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export const TelemetryTab = ({ deviceId, deviceTypeName }: TelemetryTabProps) => {
  const { data: telemetryRecords = [], isLoading, isError } = useDeviceTelemetry(deviceId, { limit: 50 });
  const profile = getProfile(deviceTypeName);

  const latest = telemetryRecords[0];
  const latestFlat = latest ? flattenRecord(latest) : null;
  const latestCustom = parseCustomJson(latest?.custom_json);

  const chartData = telemetryRecords
    .slice()
    .reverse()
    .map((r) => flattenRecord(r));

  if (isLoading) return <Spinner />;

  if (isError) {
    return (
      <Callout intent="danger" icon="error">
        Failed to load telemetry data. Try refreshing the page.
      </Callout>
    );
  }

  if (telemetryRecords.length === 0) {
    return (
      <Callout icon="info-sign" intent="primary">
        No telemetry data available for this device.
      </Callout>
    );
  }

  return (
    <div className="telemetry-tab">
      {/* Current values */}
      <div className="telemetry-section">
        <span className="section-label">Current Values</span>
        <div className="telemetry-current-table">
          {profile.currentValues.map((metric) => {
            const value = latestFlat?.[metric.key];
            return (
              <div key={metric.key} className="telemetry-current-row">
                <span className="telemetry-current-key">{metric.label}</span>
                <span className="telemetry-current-val mono-data" style={{ color: metric.color }}>
                  {value != null ? `${formatValue(value)} ${metric.unit}`.trim() : '—'}
                </span>
              </div>
            );
          })}
          {profile.infoFields?.map((field) => {
            const value = latestCustom[field.key];
            return (
              <div key={field.key} className="telemetry-current-row">
                <span className="telemetry-current-key">{field.label}</span>
                <span className="telemetry-current-val mono-data">{value ?? '—'}</span>
              </div>
            );
          })}
        </div>
        {latest && (
          <div className="telemetry-current-timestamp">
            Last updated {formatTimestamp(latest.received_at)}
          </div>
        )}
      </div>

      {/* Charts */}
      <div className="telemetry-section">
        <span className="section-label">Charts</span>
        {profile.charts.map((metric) => {
          const latestValue = chartData[chartData.length - 1]?.[metric.key];
          const yDomain: [number | 'auto', number | 'auto'] = isPercentMetric(metric.unit)
            ? [0, 100]
            : ['auto', 'auto'];
          return (
            <div key={metric.key} className="telemetry-chart">
              <div className="telemetry-header">
                <span className="section-label">{metric.label}</span>
                <span className="mono-data" style={{ fontSize: 14, color: metric.color }}>
                  {latestValue != null ? `${formatValue(latestValue)}${metric.unit}` : '—'}
                </span>
              </div>
              <ResponsiveContainer width="100%" height={160}>
                <AreaChart data={chartData} margin={{ top: 4, right: 8, bottom: 0, left: -12 }}>
                  <defs>
                    <linearGradient id={`tel-${metric.key}`} x1="0" y1="0" x2="0" y2="1">
                      <stop offset="0%" stopColor={metric.color} stopOpacity={0.3} />
                      <stop offset="100%" stopColor={metric.color} stopOpacity={0} />
                    </linearGradient>
                  </defs>
                  <CartesianGrid
                    strokeDasharray="3 3"
                    stroke="rgba(255,255,255,0.06)"
                    vertical={false}
                  />
                  <XAxis
                    dataKey="received_at"
                    tickFormatter={formatAxisTime}
                    tick={{ fontSize: 11, fill: 'rgba(255,255,255,0.4)' }}
                    axisLine={{ stroke: 'rgba(255,255,255,0.08)' }}
                    tickLine={false}
                    minTickGap={40}
                  />
                  <YAxis
                    domain={yDomain}
                    tick={{ fontSize: 11, fill: 'rgba(255,255,255,0.4)' }}
                    axisLine={false}
                    tickLine={false}
                    width={40}
                    tickFormatter={(v: number) => `${v}${metric.unit}`}
                  />
                  <Tooltip
                    content={<ChartTooltip metric={metric} />}
                    cursor={{ stroke: 'rgba(255,255,255,0.15)' }}
                    isAnimationActive={false}
                  />
                  <Area
                    type="monotone"
                    dataKey={metric.key}
                    stroke={metric.color}
                    strokeWidth={1.5}
                    fill={`url(#tel-${metric.key})`}
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

      {/* Historical values table */}
      <div className="telemetry-section">
        <span className="section-label">Historical Values</span>
        <div className="telemetry-table-wrap">
          <table className="telemetry-table">
            <thead>
              <tr>
                <th>Time</th>
                {profile.tableColumns.map((col) => (
                  <th key={col.key}>{col.label}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {telemetryRecords.map((r) => {
                const flat = flattenRecord(r);
                return (
                  <tr key={r.id}>
                    <td className="mono-data">{formatTimestamp(r.received_at)}</td>
                    {profile.tableColumns.map((col) => (
                      <td key={col.key} className="mono-data">{formatValue(flat[col.key])}</td>
                    ))}
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
};
