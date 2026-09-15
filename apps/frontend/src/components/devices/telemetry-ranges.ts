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
