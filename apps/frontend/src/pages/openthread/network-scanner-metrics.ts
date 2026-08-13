import type { ThreadChannelDiagnostics, ThreadRadioStatistics } from '../../types/api';

export type ConditionTone = 'excellent' | 'good' | 'fair' | 'poor' | 'unknown';

export interface NetworkCondition {
  label: string;
  detail: string;
  tone: ConditionTone;
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(Math.max(value, minimum), maximum);
}

export function utilizationCondition(
  utilization: number | null | undefined,
  ccaFailureRate?: number | null,
): NetworkCondition {
  const measurements = [utilization, ccaFailureRate].filter(
    (value): value is number => value != null,
  );
  if (measurements.length === 0) {
    return { label: 'Collecting', detail: 'Waiting for radio samples', tone: 'unknown' };
  }

  const pressure = Math.max(...measurements);
  if (pressure < 20) {
    return {
      label: 'Excellent',
      detail: 'Clear airtime with little contention',
      tone: 'excellent',
    };
  }
  if (pressure < 45) {
    return { label: 'Good', detail: 'Normal activity and low contention', tone: 'good' };
  }
  if (pressure < 70) {
    return { label: 'Busy', detail: 'Elevated activity may affect latency', tone: 'fair' };
  }
  return { label: 'Congested', detail: 'High contention on the active channel', tone: 'poor' };
}

export function signalCondition(rssi: number | null | undefined): NetworkCondition {
  if (rssi == null) {
    return { label: 'No sample', detail: 'No packet RSSI is available yet', tone: 'unknown' };
  }
  if (rssi >= -55) {
    return { label: 'Excellent', detail: 'Very strong received signal', tone: 'excellent' };
  }
  if (rssi >= -70) {
    return { label: 'Good', detail: 'Reliable received signal', tone: 'good' };
  }
  if (rssi >= -82) {
    return { label: 'Fair', detail: 'Usable with reduced link margin', tone: 'fair' };
  }
  return { label: 'Weak', detail: 'Low link margin; placement may help', tone: 'poor' };
}

function channelScore(channel: ThreadChannelDiagnostics): number | null {
  const hasMeasurement =
    channel.utilization_percent != null ||
    channel.max_rssi_dbm != null ||
    channel.network_count > 0;
  if (!hasMeasurement) return null;

  const utilization = channel.utilization_percent ?? 0;
  const energyPressure =
    channel.max_rssi_dbm == null ? 0 : clamp(((channel.max_rssi_dbm + 100) / 65) * 100, 0, 100);
  return utilization * 0.65 + energyPressure * 0.25 + channel.network_count * 10;
}

export function recommendedChannel(channels: ThreadChannelDiagnostics[]): number | null {
  let recommendation: { channel: number; score: number } | null = null;
  for (const channel of channels) {
    const score = channelScore(channel);
    if (score == null) continue;
    if (!recommendation || score < recommendation.score) {
      recommendation = { channel: channel.channel, score };
    }
  }
  return recommendation?.channel ?? null;
}

export function percentageRate(
  numerator: number | null,
  denominator: number | null,
): number | null {
  if (numerator == null || denominator == null || denominator <= 0) return null;
  return (numerator / denominator) * 100;
}

export function combinedErrorRate(statistics: ThreadRadioStatistics): number | null {
  const errors = (statistics.tx_errors ?? 0) + (statistics.rx_errors ?? 0);
  const frames = (statistics.tx_total ?? 0) + (statistics.rx_total ?? 0);
  if (statistics.tx_errors == null && statistics.rx_errors == null) return null;
  return percentageRate(errors, frames);
}

export function energyLevel(rssi: number | null): number {
  if (rssi == null) return 0;
  return clamp(((rssi + 100) / 70) * 100, 0, 100);
}
