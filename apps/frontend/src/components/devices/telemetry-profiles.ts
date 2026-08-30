// ---------------------------------------------------------------------------
// Per-device-type metric profiles
// ---------------------------------------------------------------------------

export interface MetricDef {
  label: string;
  /** Key into the flattened telemetry row (top-level field or custom_json key) */
  key: string;
  color: string;
  unit: string;
}

export interface InfoField {
  label: string;
  key: string;
}

export interface DeviceProfile {
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

export function getProfile(deviceTypeName: string): DeviceProfile {
  return PROFILES[deviceTypeName] ?? DEFAULT_PROFILE;
}

// ---------------------------------------------------------------------------
// Range configuration
// ---------------------------------------------------------------------------

export type RangeKey = '15m' | '1h' | '6h' | '24h' | '7d' | '30d' | 'all';

interface RangeConfig {
  label: string;
  offsetMs: number | null; // null = "All"
  limit: number;
}

export const RANGES: Record<RangeKey, RangeConfig> = {
  '15m': { label: '15m', offsetMs: 15 * 60 * 1000, limit: 200 },
  '1h': { label: '1h', offsetMs: 60 * 60 * 1000, limit: 500 },
  '6h': { label: '6h', offsetMs: 6 * 60 * 60 * 1000, limit: 1000 },
  '24h': { label: '24h', offsetMs: 24 * 60 * 60 * 1000, limit: 2000 },
  '7d': { label: '7d', offsetMs: 7 * 24 * 60 * 60 * 1000, limit: 1000 },
  '30d': { label: '30d', offsetMs: 30 * 24 * 60 * 60 * 1000, limit: 1000 },
  all: { label: 'All', offsetMs: null, limit: 1000 },
};

export const RANGE_OPTIONS = (Object.keys(RANGES) as RangeKey[]).map((key) => ({
  label: RANGES[key].label,
  value: key,
}));

export function computeSince(range: (typeof RANGES)[RangeKey]): string | undefined {
  if (range.offsetMs == null) return undefined;
  return new Date(Date.now() - range.offsetMs).toISOString();
}
