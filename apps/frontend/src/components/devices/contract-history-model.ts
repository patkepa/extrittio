import type { DeviceMetric } from '../../types/api';

const FALLBACK_COLORS = ['#2965CC', '#0F9960', '#D99E0B', '#D13913', '#7961DB', '#238551'];

function metricKey(streamKey: string, fieldPath: string): string {
  return `${streamKey}.${fieldPath}`;
}

export interface HistoryMetricDefinition {
  key: string;
  blueprintRevisionId: string;
  blueprintName: string;
  blueprintRevision: number;
  streamKey: string;
  fieldPath: string;
  valueType: string;
  label: string;
  unit?: string;
  color: string;
  presentation?: {
    color?: string;
    chart?: 'line' | 'step' | 'bar' | 'none';
    precision?: number;
  };
}

export interface HistoryEventRow {
  eventId: string;
  occurredAt: string;
  blueprintName: string;
  blueprintRevision: number;
  values: Record<string, DeviceMetric['value']>;
}

export function historyMetricKey(
  metric: Pick<DeviceMetric, 'blueprint_revision_id' | 'stream_key' | 'field_path'>,
): string {
  return JSON.stringify([metric.blueprint_revision_id, metric.stream_key, metric.field_path]);
}

export function historyMetricDefinitions(metrics: DeviceMetric[]): HistoryMetricDefinition[] {
  const definitions = new Map<string, HistoryMetricDefinition>();
  for (const metric of metrics) {
    const key = historyMetricKey(metric);
    if (definitions.has(key)) continue;
    const source = metric.field_presentation;
    const presentation = source
      ? {
          color: source.color ?? undefined,
          chart: source.chart ?? undefined,
          precision: source.precision ?? undefined,
        }
      : undefined;
    definitions.set(key, {
      key,
      blueprintRevisionId: metric.blueprint_revision_id,
      blueprintName: metric.blueprint_name,
      blueprintRevision: metric.blueprint_revision,
      streamKey: metric.stream_key,
      fieldPath: metric.field_path,
      valueType: metric.value_type,
      label: metric.field_label,
      unit: metric.field_unit ?? undefined,
      color: presentation?.color ?? FALLBACK_COLORS[definitions.size % FALLBACK_COLORS.length]!,
      presentation: presentation ?? undefined,
    });
  }
  return [...definitions.values()];
}

export function historyEventRows(metrics: DeviceMetric[]): HistoryEventRow[] {
  const rows = new Map<string, HistoryEventRow>();
  for (const metric of metrics) {
    const row = rows.get(metric.event_id) ?? {
      eventId: metric.event_id,
      occurredAt: metric.occurred_at,
      blueprintName: metric.blueprint_name,
      blueprintRevision: metric.blueprint_revision,
      values: {},
    };
    row.values[historyMetricKey(metric)] = metric.value;
    rows.set(metric.event_id, row);
  }
  return [...rows.values()].sort(
    (left, right) => new Date(right.occurredAt).getTime() - new Date(left.occurredAt).getTime(),
  );
}

export function currentContractValues(
  metrics: DeviceMetric[],
  contractId: string,
): Record<string, DeviceMetric['value']> {
  const values: Record<string, DeviceMetric['value']> = {};
  const latest = new Map<string, DeviceMetric>();
  for (const metric of metrics) {
    if (metric.contract_id !== contractId) continue;
    const key = metricKey(metric.stream_key, metric.field_path);
    const previous = latest.get(key);
    if (
      !previous ||
      Date.parse(metric.occurred_at) > Date.parse(previous.occurred_at) ||
      (metric.occurred_at === previous.occurred_at && metric.event_id > previous.event_id)
    ) {
      latest.set(key, metric);
      values[key] = metric.value;
    }
  }
  return values;
}

export function latestCurrentMetric(
  metrics: DeviceMetric[],
  contractId: string,
): DeviceMetric | undefined {
  return metrics
    .filter((metric) => metric.contract_id === contractId)
    .reduce<
      DeviceMetric | undefined
    >((latest, metric) => (!latest || Date.parse(metric.occurred_at) > Date.parse(latest.occurred_at) ? metric : latest), undefined);
}

export function historyChartSamples(metrics: DeviceMetric[], definition: HistoryMetricDefinition) {
  return metrics
    .flatMap((metric): Record<string, string | number | null>[] => {
      if (
        metric.blueprint_revision_id !== definition.blueprintRevisionId ||
        metric.stream_key !== definition.streamKey ||
        metric.field_path !== definition.fieldPath ||
        metric.value_type !== definition.valueType
      )
        return [];
      const value =
        typeof metric.value === 'string' && metric.value_type === 'int64'
          ? Number(metric.value)
          : metric.value;
      if (
        typeof value !== 'number' ||
        !Number.isFinite(value) ||
        (metric.value_type === 'int64' && !Number.isSafeInteger(value))
      )
        return [];
      return [{ occurred_at: metric.occurred_at, [definition.key]: value }];
    })
    .sort(
      (left, right) =>
        new Date(String(left.occurred_at)).getTime() -
        new Date(String(right.occurred_at)).getTime(),
    );
}

export function historyHasUnplottableValues(
  metrics: DeviceMetric[],
  definition: HistoryMetricDefinition,
): boolean {
  return metrics.some(
    (metric) =>
      metric.blueprint_revision_id === definition.blueprintRevisionId &&
      metric.stream_key === definition.streamKey &&
      metric.field_path === definition.fieldPath &&
      metric.value_type === definition.valueType &&
      metric.value_type === 'int64' &&
      typeof metric.value === 'string' &&
      !Number.isSafeInteger(Number(metric.value)),
  );
}
