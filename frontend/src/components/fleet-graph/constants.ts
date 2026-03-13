// --- Status colors (used by graph nodes and health panel) ---
export const STATUS_COLORS: Record<string, string> = {
  online: '#0F9960',
  offline: '#E76A6E',
  warning: '#D9822B',
};

export const FLEET_COLOR = '#2D72D2';
export const DEFAULT_COLOR = '#555555';

// --- Health tier thresholds (milliseconds) ---
export const STALENESS_FRESH_MS = 30_000;       // < 30s
export const STALENESS_WARM_MS = 300_000;        // < 5m
export const STALENESS_STALE_MS = 1_800_000;     // < 30m
// > 30m = dead

// --- Health tier colors ---
export const TIER_COLORS = {
  fresh: '#0F9960',   // Bright green
  warm: '#D9822B',    // Amber
  stale: '#C87619',   // Orange
  dead: '#E76A6E',    // Deep red
} as const;

export type HealthTier = keyof typeof TIER_COLORS;

// --- Device type abbreviations ---
export const TYPE_ABBREVS: Record<string, string> = {
  'mac-device': 'M',
  linux: 'L',
  esp32: 'E',
};
