// --- Status colors (used by graph nodes and health panel) ---
export const STATUS_COLORS: Record<string, string> = {
  online: '#0F9960',
  offline: '#E76A6E',
  warning: '#D9822B',
};

export const FLEET_COLOR = '#2D72D2';
export const SELECTION_COLOR = '#2D72D2';
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

// --- Device type abbreviations (fallback for labels) ---
export const TYPE_ABBREVS: Record<string, string> = {
  'mac-device': 'M',
  linux: 'L',
  esp32: 'E',
};

// --- Device type Blueprint.js 16px icon SVG paths ---
// Each value is an array of `d` attribute strings for a 16×16 viewBox.
// Icons: desktop (mac), console (linux), sim-card (esp32), widget (fallback)
export const TYPE_ICON_PATHS: Record<string, string[]> = {
  'mac-device': [
    'M15 0H1C.45 0 0 .45 0 1v10c0 .55.45 1 1 1h4.75l-.5 2H4c-.55 0-1 .45-1 1s.45 1 1 1h8c.55 0 1-.45 1-1s-.45-1-1-1h-1.25l-.5-2H15c.55 0 1-.45 1-1V1c0-.55-.45-1-1-1zm-1 10H2V2h12v8z',
  ],
  linux: [
    'M15 15H1c-.55 0-1-.45-1-1V2c0-.55.45-1 1-1h14c.55 0 1 .45 1 1v12c0 .55-.45 1-1 1zM14 5H2v8h12V5zM4 6c.28 0 .53.11.71.29l2 2c.18.18.29.43.29.71s-.11.53-.29.71l-2 2a1.003 1.003 0 01-1.42-1.42L4.59 9l-1.3-1.29A1.003 1.003 0 014 6zm5 4h3c.55 0 1 .45 1 1s-.45 1-1 1H9c-.55 0-1-.45-1-1s.45-1 1-1z',
  ],
  esp32: [
    'M13.71 4.29l-4-4A.997.997 0 009 0H3c-.55 0-1 .45-1 1v14c0 .55.45 1 1 1h10c.55 0 1-.45 1-1V5c0-.28-.11-.53-.29-.71zM7 6h2v2H7V6zM4 6h2v2H4V6zm2 8H4v-2h2v2zm3 0H7v-2h2v2zm3 0h-2v-2h2v2zm0-3H4V9h8v2zm0-3h-2V6h2v2z',
  ],
};

export const FALLBACK_ICON_PATHS: string[] = [
  'M13 11h2V5h-2v6zM3 5H1v6h2V5zm11-1c1.1 0 2-.9 2-2s-.9-2-2-2-2 .9-2 2 .9 2 2 2zM2 12c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2zm12 0c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2zM5 3h6V1H5v2zM2 0C.9 0 0 .9 0 2s.9 2 2 2 2-.9 2-2-.9-2-2-2zm3 15h6v-2H5v2z',
];
