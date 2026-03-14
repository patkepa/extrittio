import { useState, useMemo } from 'react';
import { Callout, SegmentedControl, Spinner } from '@blueprintjs/core';
import { UPlotChart } from '../charts/UPlot';
import { toAlignedData, tooltipPlugin } from '../charts/uplot-helpers';
import { useDeviceTelemetry, useAllDeviceTelemetry } from '../../hooks/use-telemetry';
import type { TelemetryRecord } from '../../types/api';
import type uPlot from 'uplot';
import { getProfile, RANGES, RANGE_OPTIONS, computeSince } from './telemetry-profiles';
import type { RangeKey, MetricDef } from './telemetry-profiles';
import { hexToRgba } from '../../utils/color';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatTimestamp(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleString(undefined, {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
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

/** Whether a metric should use a 0-100 domain (percentage metrics) */
function isPercentMetric(unit: string): boolean {
  return unit === '%';
}


/** Format unix seconds to locale time string */
function formatTooltipTime(unixSec: number): string {
  return formatTimestamp(new Date(unixSec * 1000).toISOString());
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

interface TelemetryTabProps {
  deviceId: string;
  deviceTypeName: string;
}

export const TelemetryTab = ({ deviceId, deviceTypeName }: TelemetryTabProps) => {
  const [selectedRange, setSelectedRange] = useState<RangeKey>('24h');
  const rangeConfig = RANGES[selectedRange];
  const isAll = selectedRange === 'all';

  // Stabilize `since` so the React Query key doesn't change on every render
  const since = useMemo(() => computeSince(rangeConfig), [rangeConfig]);
  const boundedQuery = useDeviceTelemetry(isAll ? null : deviceId, {
    limit: rangeConfig.limit,
    since,
  });

  // Paginated query (for "All" only)
  const allQuery = useAllDeviceTelemetry(isAll ? deviceId : null);

  // Pick the active query result
  const activeQuery = isAll ? allQuery : boundedQuery;
  const telemetryRecords = useMemo(() => activeQuery.data ?? [], [activeQuery.data]);
  const isLoading = activeQuery.isLoading;
  const isFetching = activeQuery.isFetching;
  const isError = activeQuery.isError;

  const profile = getProfile(deviceTypeName);

  // "All" data is sorted ascending; others are descending. Latest is always newest.
  const latest = isAll ? telemetryRecords[telemetryRecords.length - 1] : telemetryRecords[0];
  const latestFlat = latest ? flattenRecord(latest) : null;
  const latestCustom = parseCustomJson(latest?.custom_json);

  // "All" data is pre-sorted ascending from the hook; others need reversing
  const chartData = useMemo(() => {
    const records = isAll
      ? telemetryRecords.map((r) => flattenRecord(r))
      : telemetryRecords
          .slice()
          .reverse()
          .map((r) => flattenRecord(r));
    return records;
  }, [telemetryRecords, isAll]);

  if (isLoading) return <Spinner />;

  if (isError) {
    return (
      <Callout intent="danger" icon="error">
        Failed to load telemetry data. Try refreshing the page.
      </Callout>
    );
  }

  return (
    <div className="telemetry-tab">
      {/* Current values section */}
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

      {/* Charts section with range selector */}
      <div className="telemetry-section">
        <div className="telemetry-range-bar">
          <span className="section-label">Charts</span>
          <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
            {isFetching && !isLoading && <Spinner size={16} />}
            <SegmentedControl
              options={RANGE_OPTIONS}
              value={selectedRange}
              onValueChange={(val) => setSelectedRange(val as RangeKey)}
              small
            />
          </div>
        </div>
        {isAll && telemetryRecords.length >= 50_000 && (
          <Callout intent="warning" icon="info-sign" compact style={{ marginTop: 8 }}>
            Showing most recent 50,000 records.
          </Callout>
        )}

        {telemetryRecords.length === 0 ? (
          <Callout icon="info-sign" intent="primary" style={{ marginTop: 12 }}>
            No telemetry data in this time range.
          </Callout>
        ) : (
          profile.charts.map((metric) => (
            <TelemetryChart key={metric.key} metric={metric} chartData={chartData} />
          ))
        )}
      </div>

      {/* Historical values table — newest first for all ranges */}
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
              {(isAll ? [...telemetryRecords].reverse() : telemetryRecords).map((r) => {
                const flat = flattenRecord(r);
                return (
                  <tr key={r.id}>
                    <td className="mono-data">{formatTimestamp(r.received_at)}</td>
                    {profile.tableColumns.map((col) => (
                      <td key={col.key} className="mono-data">
                        {formatValue(flat[col.key])}
                      </td>
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

// ---------------------------------------------------------------------------
// Individual chart for a single metric (memoized options)
// ---------------------------------------------------------------------------

function TelemetryChart({
  metric,
  chartData,
}: {
  metric: MetricDef;
  chartData: Record<string, number | string | null>[];
}) {
  const latestValue = chartData[chartData.length - 1]?.[metric.key];

  const plotData = useMemo(
    () => toAlignedData(chartData, 'received_at', [metric.key]),
    [chartData, metric.key],
  );

  const opts = useMemo((): Omit<uPlot.Options, 'width' | 'height'> => {
    const yRange: uPlot.Range.MinMax | undefined = isPercentMetric(metric.unit)
      ? [0, 100]
      : undefined;

    return {
      cursor: {
        x: true,
        y: false,
        drag: { x: false, y: false },
      },
      legend: { show: false },
      axes: [
        {
          stroke: 'rgba(255,255,255,0.4)',
          font: '11px system-ui',
          ticks: { stroke: 'rgba(255,255,255,0.06)', width: 1 },
          grid: { show: false },
          gap: 8,
        },
        {
          stroke: 'rgba(255,255,255,0.4)',
          font: '11px system-ui',
          ticks: { show: false },
          grid: { stroke: 'rgba(255,255,255,0.06)', width: 1, dash: [3, 3] },
          gap: 4,
          size: 48,
          values: (_u: uPlot, vals: number[]) => vals.map((v) => `${v}${metric.unit}`),
        },
      ],
      scales: {
        y: yRange ? { range: () => yRange } : {},
      },
      series: [
        {},
        {
          stroke: metric.color,
          width: 1.5,
          fill: hexToRgba(metric.color, 0.15),
          points: { show: false },
          spanGaps: true,
        },
      ],
      plugins: [
        tooltipPlugin((_seriesIdx, val) => `${formatValue(val)} ${metric.unit}`, formatTooltipTime),
      ],
    };
  }, [metric.color, metric.unit]);

  return (
    <div className="telemetry-chart">
      <div className="telemetry-header">
        <span className="section-label">{metric.label}</span>
        <span className="mono-data" style={{ fontSize: 14, color: metric.color }}>
          {latestValue != null ? `${formatValue(latestValue)}${metric.unit}` : '—'}
        </span>
      </div>
      <UPlotChart options={opts} data={plotData} height={160} />
    </div>
  );
}
