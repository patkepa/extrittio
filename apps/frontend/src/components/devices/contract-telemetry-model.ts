import type { DeviceContract } from '../../types/api';

const FALLBACK_COLORS = ['#2965CC', '#0F9960', '#D99E0B', '#D13913', '#7961DB', '#238551'];

export interface ContractField {
  valueType: 'float64' | 'int64' | 'string' | 'boolean' | 'json';
  label: string;
  unit?: string;
  presentation?: {
    color?: string;
    chart?: 'line' | 'step' | 'bar' | 'none';
    precision?: number;
  };
}

interface ContractStream {
  fields?: Record<string, ContractField>;
}

interface ContractDocument {
  streams?: Record<string, ContractStream>;
  presentation?: {
    summary?: Array<{ metric: string }>;
  };
}

export interface MetricDefinition extends ContractField {
  key: string;
  streamKey: string;
  fieldPath: string;
  color: string;
}

export function metricKey(streamKey: string, fieldPath: string): string {
  return `${streamKey}.${fieldPath}`;
}

export function contractMetricDefinitions(contract: DeviceContract): MetricDefinition[] {
  const document = contract.document as ContractDocument;
  const definitions = Object.entries(document.streams ?? {}).flatMap(([streamKey, stream]) =>
    Object.entries(stream.fields ?? {}).map(([fieldPath, field]) => ({
      ...field,
      key: metricKey(streamKey, fieldPath),
      streamKey,
      fieldPath,
      color: field.presentation?.color ?? '#2965CC',
    })),
  );
  const summaryOrder = new Map(
    (document.presentation?.summary ?? []).map(({ metric }, index) => [metric, index]),
  );
  return definitions
    .sort((left, right) => {
      const leftOrder = summaryOrder.get(left.key) ?? Number.MAX_SAFE_INTEGER;
      const rightOrder = summaryOrder.get(right.key) ?? Number.MAX_SAFE_INTEGER;
      return leftOrder - rightOrder || left.key.localeCompare(right.key);
    })
    .map((definition, index) => ({
      ...definition,
      color:
        definition.presentation?.color ??
        FALLBACK_COLORS[index % FALLBACK_COLORS.length] ??
        '#2965CC',
    }));
}
