import { useState, useMemo } from 'react';
import axios from 'axios';
import { Button, Callout, HTMLSelect, SegmentedControl, Spinner } from '@blueprintjs/core';
import { UPlotChart, type UPlotXRange } from '../charts/UPlot';
import { toAlignedData, tooltipPlugin } from '../charts/uplot-helpers';
import { useDeviceTelemetry, useAllDeviceTelemetry } from '../../hooks/use-telemetry';
import type { TelemetryRecord } from '../../types/api';
import type uPlot from 'uplot';
import { getProfile, RANGES, RANGE_OPTIONS, computeSince } from './telemetry-profiles';
import type { RangeKey, MetricDef } from './telemetry-profiles';
import { hexToRgba } from '../../utils/color';
import { useDeviceContract } from '../../hooks/use-devices';
import { ContractTelemetryTab } from './contract-telemetry-tab';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const DEFAULT_REFRESH_INTERVAL_MS = 10_000;
const REFRESH_INTERVAL_STORAGE_KEY = 'extrittio.telemetry-refresh-interval.v1';
const REFRESH_INTERVAL_OPTIONS = [
  { label: 'Off', value: 'off' },
  { label: 'Every 1 sec', value: '1000' },
  { label: 'Every 5 sec', value: '5000' },
  { label: 'Every 10 sec', value: '10000' },
  { label: 'Every 30 sec', value: '30000' },
  { label: 'Every 1 min', value: '60000' },
  { label: 'Every 5 min', value: '300000' },
];
const REFRESH_INTERVAL_VALUES = new Set(REFRESH_INTERVAL_OPTIONS.map(({ value }) => value));

function loadRefreshInterval(): number | false {
  try {
    const stored = window.localStorage.getItem(REFRESH_INTERVAL_STORAGE_KEY);
    if (!stored || !REFRESH_INTERVAL_VALUES.has(stored)) return DEFAULT_REFRESH_INTERVAL_MS;
    return stored === 'off' ? false : Number(stored);
  } catch {
    return DEFAULT_REFRESH_INTERVAL_MS;
  }
}

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
function parseCustomJson(raw: unknown): Record<string, unknown> {
  if (!raw) return {};
  if (typeof raw === 'object' && !Array.isArray(raw)) return raw as Record<string, unknown>;
  if (typeof raw !== 'string') return {};
  try {
    const parsed = JSON.parse(raw);
    if (typeof parsed === 'object' && parsed !== null && !Array.isArray(parsed)) {
      return parsed as Record<string, unknown>;
    }
    return {};
  } catch {
    return {};
  }
}

function flattenValue(value: unknown): number | string | null {
  if (value == null) return null;
  if (typeof value === 'number') return value;
  if (typeof value === 'string') {
    const n = parseFloat(value);
    return isNaN(n) ? value : n;
  }
  if (typeof value === 'boolean') return value ? 'true' : 'false';
  return JSON.stringify(value);
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
    flat[k] = flattenValue(v);
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
  const contractQuery = useDeviceContract(deviceId, { retry: false });
  if (contractQuery.isLoading) return <Spinner />;
  if (contractQuery.data) {
    return <ContractTelemetryTab deviceId={deviceId} contract={contractQuery.data} />;
  }
  const isLegacyDevice =
    axios.isAxiosError(contractQuery.error) && contractQuery.error.response?.status === 404;
  if (contractQuery.isError && !isLegacyDevice) {
    return (
      <Callout intent="danger" icon="error">
        Failed to load the assigned device contract. Try refreshing the page.
      </Callout>
    );
  }
  return <LegacyTelemetryTab deviceId={deviceId} deviceTypeName={deviceTypeName} />;
};

const LegacyTelemetryTab = ({ deviceId, deviceTypeName }: TelemetryTabProps) => {
  const [selectedRange, setSelectedRange] = useState<RangeKey>('24h');
  const [refreshInterval, setRefreshInterval] = useState<number | false>(loadRefreshInterval);
  const [zoomRange, setZoomRange] = useState<UPlotXRange | null>(null);
  const rangeConfig = RANGES[selectedRange];
  const isAll = selectedRange === 'all';

  // Stabilize `since` so the React Query key doesn't change on every render
  const since = useMemo(() => computeSince(rangeConfig), [rangeConfig]);
  const boundedQuery = useDeviceTelemetry(
    isAll ? null : deviceId,
    {
      limit: rangeConfig.limit,
      since,
    },
    {
      refetchInterval: refreshInterval,
    },
  );

  // Paginated query (for "All" only)
  const allQuery = useAllDeviceTelemetry(isAll ? deviceId : null);

  // Pick the active query result
  const activeQuery = isAll ? allQuery : boundedQuery;
  const telemetryRecords = useMemo(() => activeQuery.data ?? [], [activeQuery.data]);
  const isLoading = activeQuery.isLoading;
  const isFetching = activeQuery.isFetching;
  const isError = activeQuery.isError;

  // "All" data is sorted ascending; others are descending. Latest is always newest.
  const latest = isAll ? telemetryRecords[telemetryRecords.length - 1] : telemetryRecords[0];
  const latestFlat = latest ? flattenRecord(latest) : null;
  const latestCustom = parseCustomJson(latest?.custom_json);
  const profile = getProfile(deviceTypeName);

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
            const value = flattenValue(latestCustom[field.key]);
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
          <div className="telemetry-range-controls">
            <span className="telemetry-zoom-status" role="status">
              {zoomRange ? 'Zoomed time range' : 'Drag a chart to zoom'}
            </span>
            {zoomRange && (
              <Button icon="zoom-out" small minimal onClick={() => setZoomRange(null)}>
                Reset zoom
              </Button>
            )}
            {isFetching && !isLoading && <Spinner size={16} />}
            <label className="telemetry-refresh-control">
              <span>Auto-refresh</span>
              <HTMLSelect
                aria-label="Auto-refresh interval"
                disabled={isAll}
                title={isAll ? 'Auto-refresh is disabled for the All range' : undefined}
                value={refreshInterval === false ? 'off' : String(refreshInterval)}
                options={REFRESH_INTERVAL_OPTIONS}
                onChange={(event) => {
                  const value = event.target.value;
                  const nextInterval = value === 'off' ? false : Number(value);
                  setRefreshInterval(nextInterval);
                  try {
                    window.localStorage.setItem(REFRESH_INTERVAL_STORAGE_KEY, value);
                  } catch {
                    // Keep the in-memory preference when storage is unavailable.
                  }
                }}
                minimal
              />
            </label>
            <SegmentedControl
              options={RANGE_OPTIONS}
              value={selectedRange}
              onValueChange={(val) => {
                setSelectedRange(val as RangeKey);
                setZoomRange(null);
              }}
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
          <>
            {profile.charts.map((metric) => (
              <TelemetryChart
                key={metric.key}
                metric={metric}
                chartData={chartData}
                zoomRange={zoomRange}
                onZoomRangeChange={setZoomRange}
              />
            ))}
          </>
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
  zoomRange,
  onZoomRangeChange,
}: {
  metric: MetricDef;
  chartData: Record<string, number | string | null>[];
  zoomRange: UPlotXRange | null;
  onZoomRangeChange: (range: UPlotXRange | null) => void;
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
      <UPlotChart
        options={opts}
        data={plotData}
        height={160}
        zoomable
        xRange={zoomRange}
        onXRangeChange={onZoomRangeChange}
      />
    </div>
  );
}
