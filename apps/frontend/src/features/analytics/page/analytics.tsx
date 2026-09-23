import { useMemo, useState } from 'react';
import {
  Button,
  Callout,
  Checkbox,
  FormGroup,
  HTMLSelect,
  Icon,
  NonIdealState,
  Popover,
  Spinner,
} from '@blueprintjs/core';
import type { Device } from '../../../types/api';
import { useFleets } from '../../../hooks/use-fleets';
import { useAllDevices } from '../../devices/queries/use-devices';
import {
  type AnalyticsMetricCatalogEntry,
  type AnalyticsQueryRequest,
  type AnalyticsSeriesMode,
  type AnalyticsWeighting,
} from '../api/analytics-api';
import { AnalyticsChart } from '../components/analytics-chart';
import { AnalyticsSummary } from '../components/analytics-summary';
import { DeviceComparisonTable } from '../components/device-comparison-table';
import { useAnalyticsCatalog, useAnalyticsQuery } from '../queries/use-analytics';
import { formatBucket } from '../model/chart-data';
import { analyticsMetricId, groupAnalyticsMetrics } from '../model/metric-catalog';
import './analytics.css';

interface AnalyticsDraft {
  fleetId: string;
  deviceIds: string[] | null;
  metricId: string;
  rangeHours: number;
  bucketSeconds: string;
  mode: AnalyticsSeriesMode;
  weighting: AnalyticsWeighting;
}

const DEFAULT_DRAFT: AnalyticsDraft = {
  fleetId: '',
  deviceIds: null,
  metricId: '',
  rangeHours: 24,
  bucketSeconds: '',
  mode: 'mean_and_range',
  weighting: 'equal_device',
};

const EMPTY_METRICS: AnalyticsMetricCatalogEntry[] = [];

function buildRequest(
  draft: AnalyticsDraft,
  metric: AnalyticsMetricCatalogEntry,
): AnalyticsQueryRequest {
  const to = new Date();
  const from = new Date(to.getTime() - draft.rangeHours * 60 * 60 * 1000);
  return {
    scope: {
      fleet_ids: draft.fleetId ? [Number(draft.fleetId)] : [],
      device_ids: draft.deviceIds ?? [],
    },
    metric: {
      blueprint_id: metric.blueprint_id,
      stream_key: metric.stream_key,
      field_path: metric.field_path,
    },
    from: from.toISOString(),
    to: to.toISOString(),
    bucket_seconds: draft.bucketSeconds ? Number(draft.bucketSeconds) : null,
    mode: draft.mode,
    weighting: draft.weighting,
    max_points_per_series: 1_200,
  };
}

function errorMessage(error: Error | null): string {
  const candidate = error as Error & { response?: { data?: { message?: string } } };
  return candidate.response?.data?.message ?? error?.message ?? 'The analytics query failed.';
}

function deviceMatches(device: Device, fleetId: string): boolean {
  return !fleetId || device.fleet_id === Number(fleetId);
}

export function Analytics() {
  const [draft, setDraft] = useState<AnalyticsDraft>(DEFAULT_DRAFT);
  const [request, setRequest] = useState<AnalyticsQueryRequest | null>(null);
  const catalogQuery = useAnalyticsCatalog();
  const fleetsQuery = useFleets();
  const devicesQuery = useAllDevices();
  const analyticsQuery = useAnalyticsQuery(request);

  const compatibleDevices = useMemo(
    () => (devicesQuery.data?.data ?? []).filter((device) => deviceMatches(device, draft.fleetId)),
    [devicesQuery.data?.data, draft.fleetId],
  );
  const selectedDeviceSet = useMemo(
    () => new Set(draft.deviceIds ?? compatibleDevices.map((device) => device.id)),
    [compatibleDevices, draft.deviceIds],
  );
  const selectedCount = compatibleDevices.filter((device) =>
    selectedDeviceSet.has(device.id),
  ).length;
  const metrics = catalogQuery.data?.metrics ?? EMPTY_METRICS;
  const metricGroups = useMemo(() => groupAnalyticsMetrics(metrics), [metrics]);
  const selectedMetric = useMemo(
    () =>
      metrics.find((metric) => analyticsMetricId(metric) === draft.metricId) ?? metrics[0] ?? null,
    [draft.metricId, metrics],
  );

  const resetScopeSelection = (patch: Partial<AnalyticsDraft>) => {
    setDraft((current) => ({ ...current, ...patch, deviceIds: null }));
  };

  const toggleDevice = (deviceId: string) => {
    setDraft((current) => {
      const allIds = compatibleDevices.map((device) => device.id);
      const selected = new Set(current.deviceIds ?? allIds);
      if (selected.has(deviceId)) selected.delete(deviceId);
      else selected.add(deviceId);
      const nextIds = allIds.filter((id) => selected.has(id));
      return { ...current, deviceIds: nextIds.length === allIds.length ? null : nextIds };
    });
  };

  const runQuery = () => {
    if (selectedCount === 0 || !selectedMetric) return;
    const compatibleIds = new Set(compatibleDevices.map((device) => device.id));
    const deviceIds = draft.deviceIds?.filter((id) => compatibleIds.has(id)) ?? null;
    setRequest(buildRequest({ ...draft, deviceIds }, selectedMetric));
  };

  const result = analyticsQuery.data;
  const pageLoading = request != null && analyticsQuery.isPending;

  return (
    <div className="analytics-page">
      <header className="analytics-header">
        <div>
          <h3>Analytics</h3>
          <p>Compare telemetry across devices and fleets.</p>
        </div>
        <div className="analytics-header__actions">
          <HTMLSelect
            aria-label="Time range"
            value={draft.rangeHours}
            onChange={(event) =>
              setDraft((current) => ({ ...current, rangeHours: Number(event.target.value) }))
            }
          >
            <option value={1}>Last hour</option>
            <option value={6}>Last 6 hours</option>
            <option value={24}>Last 24 hours</option>
            <option value={168}>Last 7 days</option>
            <option value={720}>Last 30 days</option>
          </HTMLSelect>
          <Button
            icon="refresh"
            minimal
            aria-label="Refresh analytics"
            loading={analyticsQuery.isFetching}
            disabled={request == null}
            onClick={() => void analyticsQuery.refetch()}
          />
        </div>
      </header>

      <section className="analytics-query-bar" aria-label="Analytics query">
        <FormGroup label="Fleet">
          <HTMLSelect
            fill
            value={draft.fleetId}
            onChange={(event) => resetScopeSelection({ fleetId: event.target.value })}
          >
            <option value="">All fleets</option>
            {(fleetsQuery.data ?? []).map((fleet) => (
              <option key={fleet.id} value={fleet.id}>
                {fleet.name}
              </option>
            ))}
          </HTMLSelect>
        </FormGroup>

        <FormGroup label="Devices">
          <Popover
            placement="bottom-start"
            content={
              <div className="analytics-device-picker">
                <Checkbox
                  checked={
                    selectedCount === compatibleDevices.length && compatibleDevices.length > 0
                  }
                  indeterminate={selectedCount > 0 && selectedCount < compatibleDevices.length}
                  label={`All in scope (${compatibleDevices.length})`}
                  onChange={() =>
                    setDraft((current) => ({
                      ...current,
                      deviceIds: selectedCount === compatibleDevices.length ? [] : null,
                    }))
                  }
                />
                <div className="analytics-device-picker__list">
                  {compatibleDevices.map((device) => (
                    <Checkbox
                      key={device.id}
                      checked={selectedDeviceSet.has(device.id)}
                      label={device.name}
                      onChange={() => toggleDevice(device.id)}
                    />
                  ))}
                </div>
              </div>
            }
          >
            <Button fill rightIcon="caret-down" alignText="left">
              {selectedCount === compatibleDevices.length
                ? `All in scope (${compatibleDevices.length})`
                : `${selectedCount} selected`}
            </Button>
          </Popover>
        </FormGroup>

        <FormGroup label="Metric">
          <HTMLSelect
            fill
            value={selectedMetric ? analyticsMetricId(selectedMetric) : ''}
            disabled={catalogQuery.isPending || metrics.length === 0}
            onChange={(event) =>
              setDraft((current) => ({
                ...current,
                metricId: event.target.value,
              }))
            }
          >
            {metrics.length === 0 ? <option value="">No numeric blueprint metrics</option> : null}
            {metricGroups.map((group) => (
              <optgroup key={group.blueprintId} label={group.blueprintName}>
                {group.metrics.map((metric) => (
                  <option key={analyticsMetricId(metric)} value={analyticsMetricId(metric)}>
                    {metric.label} · {metric.stream_key}.{metric.field_path}
                  </option>
                ))}
              </optgroup>
            ))}
          </HTMLSelect>
        </FormGroup>

        <FormGroup label="View">
          <HTMLSelect
            fill
            value={draft.mode}
            onChange={(event) =>
              setDraft((current) => ({
                ...current,
                mode: event.target.value as AnalyticsSeriesMode,
              }))
            }
          >
            <option value="mean_and_range">Mean + range</option>
            <option value="fleet_mean">Fleet mean</option>
            <option value="per_device">Separate devices</option>
          </HTMLSelect>
        </FormGroup>

        <FormGroup label="Bucket">
          <HTMLSelect
            fill
            value={draft.bucketSeconds}
            onChange={(event) =>
              setDraft((current) => ({ ...current, bucketSeconds: event.target.value }))
            }
          >
            <option value="">Auto</option>
            {(catalogQuery.data?.bucket_seconds ?? [60, 300, 900, 3_600, 21_600, 86_400]).map(
              (seconds) => (
                <option key={seconds} value={seconds}>
                  {formatBucket(seconds)}
                </option>
              ),
            )}
          </HTMLSelect>
        </FormGroup>

        <Button
          className="analytics-run-button"
          icon="play"
          intent="primary"
          disabled={selectedCount === 0 || selectedMetric == null}
          loading={analyticsQuery.isFetching}
          onClick={runQuery}
        >
          Run analysis
        </Button>
      </section>

      {analyticsQuery.error ? (
        <Callout intent="danger" icon="error" title="Unable to run analytics">
          {errorMessage(analyticsQuery.error)}
        </Callout>
      ) : null}

      {!catalogQuery.isPending && metrics.length === 0 ? (
        <Callout intent="warning" icon="info-sign" title="No chartable blueprint metrics">
          Publish a device blueprint with at least one float64 or int64 stream field to use
          Analytics.
        </Callout>
      ) : null}

      {pageLoading ? (
        <div className="analytics-loading">
          <Spinner size={32} />
        </div>
      ) : result ? (
        <>
          <AnalyticsSummary result={result} />
          <section className="analytics-panel">
            <header className="analytics-panel__header">
              <div>
                <h5>
                  {result.metric.label} across {result.scope.compatible_devices} compatible device
                  {result.scope.compatible_devices === 1 ? '' : 's'}
                </h5>
                <span>
                  {result.metric.blueprint_name} · blueprint metric samples ·{' '}
                  {result.effective.weighting === 'equal_device'
                    ? 'equal device weighting'
                    : 'sample weighting'}
                </span>
              </div>
              <Popover
                placement="bottom-end"
                content={
                  <div className="analytics-weighting-menu">
                    <Button
                      minimal
                      active={draft.weighting === 'equal_device'}
                      alignText="left"
                      onClick={() =>
                        setDraft((current) => ({ ...current, weighting: 'equal_device' }))
                      }
                    >
                      Equal per device
                    </Button>
                    <Button
                      minimal
                      active={draft.weighting === 'sample'}
                      alignText="left"
                      onClick={() => setDraft((current) => ({ ...current, weighting: 'sample' }))}
                    >
                      Weight by samples
                    </Button>
                  </div>
                }
              >
                <Button minimal small icon="settings" text="Weighting" />
              </Popover>
            </header>
            {result.series.length > 0 ? (
              <AnalyticsChart result={result} />
            ) : (
              <NonIdealState
                icon="timeline-line-chart"
                title="No telemetry in this range"
                description="Try a longer time range or a different device scope."
              />
            )}
          </section>
          <DeviceComparisonTable result={result} />
          {result.warnings.map((warning) => (
            <Callout key={warning} intent="warning" icon={<Icon icon="warning-sign" />}>
              {warning}
            </Callout>
          ))}
        </>
      ) : null}
    </div>
  );
}
