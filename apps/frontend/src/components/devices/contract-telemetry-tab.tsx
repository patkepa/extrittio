import { useMemo, useState } from 'react';
import { Button, Callout, HTMLSelect, SegmentedControl, Spinner } from '@blueprintjs/core';
import type uPlot from 'uplot';
import { useAllDeviceMetrics, useDeviceMetrics } from '../../hooks/use-telemetry';
import type { DeviceContract, DeviceMetric } from '../../types/api';
import { hexToRgba } from '../../utils/color';
import { UPlotChart, type UPlotXRange } from '../charts/UPlot';
import { toAlignedData, tooltipPlugin } from '../charts/uplot-helpers';
import { computeSince, RANGE_OPTIONS, RANGES, type RangeKey } from './telemetry-profiles';
import {
  contractMetricDefinitions,
  metricKey,
  type MetricDefinition,
} from './contract-telemetry-model';

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
const MAX_ALL_METRICS = 100_000;

interface EventRow {
  eventId: string;
  occurredAt: string;
  values: Record<string, DeviceMetric['value']>;
}

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
  return new Date(iso).toLocaleString(undefined, {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  });
}

function formatValue(value: DeviceMetric['value'] | undefined, precision?: number): string {
  if (value == null) return '—';
  if (typeof value === 'number') {
    if (precision != null) return value.toFixed(precision);
    return Number.isInteger(value) ? String(value) : value.toFixed(2);
  }
  if (typeof value === 'object') return JSON.stringify(value);
  return String(value);
}

function eventRows(metrics: DeviceMetric[]): EventRow[] {
  const rows = new Map<string, EventRow>();
  for (const metric of metrics) {
    const row = rows.get(metric.event_id) ?? {
      eventId: metric.event_id,
      occurredAt: metric.occurred_at,
      values: {},
    };
    row.values[metricKey(metric.stream_key, metric.field_path)] = metric.value;
    rows.set(metric.event_id, row);
  }
  return [...rows.values()].sort(
    (left, right) => new Date(right.occurredAt).getTime() - new Date(left.occurredAt).getTime(),
  );
}

interface ContractTelemetryTabProps {
  deviceId: string;
  contract: DeviceContract;
}

export const ContractTelemetryTab = ({ deviceId, contract }: ContractTelemetryTabProps) => {
  const [selectedRange, setSelectedRange] = useState<RangeKey>('24h');
  const [refreshInterval, setRefreshInterval] = useState<number | false>(loadRefreshInterval);
  const [zoomRange, setZoomRange] = useState<UPlotXRange | null>(null);
  const range = RANGES[selectedRange];
  const isAll = selectedRange === 'all';
  const since = useMemo(() => computeSince(range), [range]);
  const definitions = useMemo(() => contractMetricDefinitions(contract), [contract]);
  const boundedQuery = useDeviceMetrics(
    isAll ? null : deviceId,
    { limit: Math.min(range.limit * Math.max(definitions.length, 1), 10_000), since },
    { refetchInterval: refreshInterval },
  );
  const allQuery = useAllDeviceMetrics(isAll ? deviceId : null);
  const query = isAll ? allQuery : boundedQuery;
  const metrics = useMemo(() => query.data ?? [], [query.data]);
  const rows = useMemo(() => eventRows(metrics), [metrics]);
  const latestValues = rows[0]?.values ?? {};
  const chartDefinitions = definitions.filter(
    ({ valueType, presentation }) =>
      (valueType === 'float64' || valueType === 'int64') && presentation?.chart !== 'none',
  );

  if (query.isLoading) return <Spinner />;
  if (query.isError) {
    return (
      <Callout intent="danger" icon="error">
        Failed to load contract-defined metrics. Try refreshing the page.
      </Callout>
    );
  }

  return (
    <div className="telemetry-tab">
      <div className="telemetry-section">
        <span className="section-label">Current Values</span>
        {definitions.length === 0 ? (
          <Callout icon="info-sign" intent="primary">
            This device contract does not declare any metric fields.
          </Callout>
        ) : (
          <div className="telemetry-current-table">
            {definitions.map((definition) => (
              <div key={definition.key} className="telemetry-current-row">
                <span className="telemetry-current-key">{definition.label}</span>
                <span
                  className="telemetry-current-val mono-data"
                  style={{ color: definition.color }}
                >
                  {formatValue(latestValues[definition.key], definition.presentation?.precision)}{' '}
                  {latestValues[definition.key] == null ? '' : definition.unit}
                </span>
              </div>
            ))}
          </div>
        )}
        {rows[0] && (
          <div className="telemetry-current-timestamp">
            Last updated {formatTimestamp(rows[0].occurredAt)}
          </div>
        )}
      </div>

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
            {query.isFetching && !query.isLoading && <Spinner size={16} />}
            <label className="telemetry-refresh-control">
              <span>Auto-refresh</span>
              <HTMLSelect
                aria-label="Auto-refresh interval"
                disabled={isAll}
                value={refreshInterval === false ? 'off' : String(refreshInterval)}
                options={REFRESH_INTERVAL_OPTIONS}
                onChange={(event) => {
                  const value = event.target.value;
                  const next = value === 'off' ? false : Number(value);
                  setRefreshInterval(next);
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
              onValueChange={(value) => {
                setSelectedRange(value as RangeKey);
                setZoomRange(null);
              }}
              small
            />
          </div>
        </div>
        {isAll && metrics.length >= MAX_ALL_METRICS && (
          <Callout intent="warning" icon="info-sign" compact style={{ marginTop: 8 }}>
            Showing the most recent {MAX_ALL_METRICS.toLocaleString()} metric samples.
          </Callout>
        )}
        {metrics.length === 0 ? (
          <Callout icon="info-sign" intent="primary" style={{ marginTop: 12 }}>
            No metric data in this time range.
          </Callout>
        ) : chartDefinitions.length === 0 ? (
          <Callout icon="info-sign" intent="primary" style={{ marginTop: 12 }}>
            The contract does not declare any numeric chart fields.
          </Callout>
        ) : (
          chartDefinitions.map((definition) => (
            <ContractMetricChart
              key={definition.key}
              definition={definition}
              metrics={metrics}
              zoomRange={zoomRange}
              onZoomRangeChange={setZoomRange}
            />
          ))
        )}
      </div>

      <div className="telemetry-section">
        <span className="section-label">Historical Values</span>
        <div className="telemetry-table-wrap">
          <table className="telemetry-table">
            <thead>
              <tr>
                <th>Time</th>
                {definitions.map((definition) => (
                  <th key={definition.key}>{definition.label}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {rows.map((row) => (
                <tr key={row.eventId}>
                  <td className="mono-data">{formatTimestamp(row.occurredAt)}</td>
                  {definitions.map((definition) => (
                    <td key={definition.key} className="mono-data">
                      {formatValue(row.values[definition.key], definition.presentation?.precision)}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
};

function ContractMetricChart({
  definition,
  metrics,
  zoomRange,
  onZoomRangeChange,
}: {
  definition: MetricDefinition;
  metrics: DeviceMetric[];
  zoomRange: UPlotXRange | null;
  onZoomRangeChange: (range: UPlotXRange | null) => void;
}) {
  const samples = useMemo(
    () =>
      metrics
        .flatMap((metric): Record<string, string | number | null>[] => {
          if (
            metric.stream_key !== definition.streamKey ||
            metric.field_path !== definition.fieldPath ||
            typeof metric.value !== 'number'
          ) {
            return [];
          }
          return [{ occurred_at: metric.occurred_at, [definition.key]: metric.value }];
        })
        .sort(
          (left, right) =>
            new Date(String(left.occurred_at)).getTime() -
            new Date(String(right.occurred_at)).getTime(),
        ),
    [definition.fieldPath, definition.key, definition.streamKey, metrics],
  );
  const plotData = useMemo(
    () => toAlignedData(samples, 'occurred_at', [definition.key]),
    [definition.key, samples],
  );
  const latest = samples[samples.length - 1]?.[definition.key];
  const unit = definition.unit ?? '';
  const options = useMemo((): Omit<uPlot.Options, 'width' | 'height'> => {
    const percentageRange: uPlot.Range.MinMax | undefined = unit.includes('%')
      ? [0, 100]
      : undefined;
    return {
      cursor: { x: true, y: false, drag: { x: false, y: false } },
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
          values: (_plot: uPlot, values: number[]) => values.map((value) => `${value}${unit}`),
        },
      ],
      scales: { y: percentageRange ? { range: () => percentageRange } : {} },
      series: [
        {},
        {
          stroke: definition.color,
          width: 1.5,
          fill: hexToRgba(definition.color, 0.15),
          points: { show: false },
          spanGaps: true,
        },
      ],
      plugins: [
        tooltipPlugin(
          (_seriesIndex, value) =>
            `${formatValue(value, definition.presentation?.precision)} ${unit}`.trim(),
          (unixSeconds) => formatTimestamp(new Date(unixSeconds * 1000).toISOString()),
        ),
      ],
    };
  }, [definition.color, definition.presentation?.precision, unit]);

  return (
    <div className="telemetry-chart">
      <div className="telemetry-header">
        <span className="section-label">{definition.label}</span>
        <span className="mono-data" style={{ fontSize: 14, color: definition.color }}>
          {latest == null
            ? '—'
            : `${formatValue(latest, definition.presentation?.precision)}${unit}`}
        </span>
      </div>
      <UPlotChart
        options={options}
        data={plotData}
        height={160}
        zoomable
        xRange={zoomRange}
        onXRangeChange={onZoomRangeChange}
      />
    </div>
  );
}
