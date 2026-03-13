import {
  STALENESS_FRESH_MS,
  STALENESS_WARM_MS,
  STALENESS_STALE_MS,
  TIER_COLORS,
  type HealthTier,
} from './constants';

/**
 * Determine health tier from staleness in milliseconds.
 * When staleness is unavailable (NaN), falls back to device status.
 */
export function getHealthTier(stalenessMs: number, status?: string): HealthTier {
  if (!Number.isNaN(stalenessMs) && stalenessMs >= 0) {
    if (stalenessMs < STALENESS_FRESH_MS) return 'fresh';
    if (stalenessMs < STALENESS_WARM_MS) return 'warm';
    if (stalenessMs < STALENESS_STALE_MS) return 'stale';
    return 'dead';
  }
  // Fallback to backend-reported status when staleness is unknown
  if (status === 'online') return 'fresh';
  if (status === 'warning') return 'warm';
  return 'dead';
}

/**
 * Linear interpolation between two hex colors.
 * `t` ranges from 0 (colorA) to 1 (colorB).
 */
function lerpColor(colorA: string, colorB: string, t: number): string {
  const a = parseInt(colorA.slice(1), 16);
  const b = parseInt(colorB.slice(1), 16);

  const rA = (a >> 16) & 0xff, gA = (a >> 8) & 0xff, bA = a & 0xff;
  const rB = (b >> 16) & 0xff, gB = (b >> 8) & 0xff, bB = b & 0xff;

  const r = Math.round(rA + (rB - rA) * t);
  const g = Math.round(gA + (gB - gA) * t);
  const bl = Math.round(bA + (bB - bA) * t);

  return `#${((r << 16) | (g << 8) | bl).toString(16).padStart(6, '0')}`;
}

/**
 * Compute the continuous staleness color from stalenessMs.
 * Interpolates within tier boundaries for smooth gradients.
 * Falls back to device status when staleness is unavailable.
 */
export function getStalenessColor(stalenessMs: number, status?: string): string {
  if (Number.isNaN(stalenessMs) || stalenessMs < 0) {
    return TIER_COLORS[getHealthTier(stalenessMs, status)];
  }

  if (stalenessMs < STALENESS_FRESH_MS) {
    return TIER_COLORS.fresh;
  }
  if (stalenessMs < STALENESS_WARM_MS) {
    const t = (stalenessMs - STALENESS_FRESH_MS) / (STALENESS_WARM_MS - STALENESS_FRESH_MS);
    return lerpColor(TIER_COLORS.fresh, TIER_COLORS.warm, t);
  }
  if (stalenessMs < STALENESS_STALE_MS) {
    const t = (stalenessMs - STALENESS_WARM_MS) / (STALENESS_STALE_MS - STALENESS_WARM_MS);
    return lerpColor(TIER_COLORS.warm, TIER_COLORS.stale, t);
  }
  return TIER_COLORS.dead;
}

/**
 * Compute pulse frequency (Hz) from health tier.
 * Fresh = 0.5Hz, warm = 0.2Hz, stale/dead = 0 (no pulse).
 */
export function getPulseFrequency(tier: HealthTier): number {
  if (tier === 'fresh') return 0.5;
  if (tier === 'warm') return 0.2;
  return 0;
}

/**
 * Compute uptime ring arc angle in radians from uptime_seconds.
 * Full circle (2*PI) at 24h+, proportional below that.
 * Minimum sliver of ~36 degrees (PI/5) for any uptime > 0.
 */
export function getUptimeArcAngle(uptimeSeconds: number): number {
  if (uptimeSeconds <= 0) return 0;
  const DAY_SECONDS = 86400;
  if (uptimeSeconds >= DAY_SECONDS) return 2 * Math.PI;
  const angle = (uptimeSeconds / DAY_SECONDS) * 2 * Math.PI;
  return Math.max(angle, Math.PI / 5); // minimum ~36 degrees
}

/**
 * Format staleness as a human-readable label (e.g., "2s ago", "5m ago").
 */
export function formatStaleness(stalenessMs: number): string {
  if (Number.isNaN(stalenessMs) || stalenessMs < 0) return 'never';
  const seconds = Math.floor(stalenessMs / 1000);
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}
