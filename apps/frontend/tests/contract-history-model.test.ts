import assert from 'node:assert/strict';
import test from 'node:test';

import {
  currentContractValues,
  historyChartSamples,
  historyEventRows,
  historyHasUnplottableValues,
  historyMetricDefinitions,
  latestCurrentMetric,
} from '../src/components/devices/contract-history-model.ts';
import type { DeviceMetric } from '../src/types/api.ts';

const sample = (
  eventId: string,
  contractId: string,
  revisionId: string,
  revision: number,
  valueType: string,
  value: number | string,
  label: string,
  unit: string,
  occurredAt: string,
): DeviceMetric => ({
  event_id: eventId,
  contract_id: contractId,
  blueprint_id: 'blueprint',
  blueprint_name: 'Machine',
  blueprint_revision_id: revisionId,
  blueprint_revision: revision,
  device_id: 'device',
  stream_key: 'readings',
  field_path: '/value',
  value_type: valueType,
  value,
  field_label: label,
  field_unit: unit,
  field_semantic: null,
  field_presentation: null,
  occurred_at: occurredAt,
});

test('separates incompatible historical revisions and uses their originating labels', () => {
  const old = sample(
    'old',
    'old-contract',
    'revision-1',
    1,
    'float64',
    21.5,
    'Temperature',
    'C',
    '2026-01-01T00:00:00Z',
  );
  const current = sample(
    'new',
    'current-contract',
    'revision-2',
    2,
    'int64',
    '22',
    'Counter',
    'ticks',
    '2026-01-02T00:00:00Z',
  );
  const metrics = [old, current];
  const definitions = historyMetricDefinitions(metrics);
  assert.equal(definitions.length, 2);
  assert.notEqual(definitions[0]?.key, definitions[1]?.key);
  assert.deepEqual(
    definitions.map(({ label, unit }) => [label, unit]),
    [
      ['Temperature', 'C'],
      ['Counter', 'ticks'],
    ],
  );
  assert.equal(historyChartSamples(metrics, definitions[0]!).length, 1);
  assert.equal(historyChartSamples(metrics, definitions[1]!).length, 1);
  const rows = historyEventRows(metrics);
  assert.equal(rows[0]?.blueprintRevision, 2);
  assert.equal(rows[0]?.values[definitions[0]!.key], undefined);
  assert.equal(rows[0]?.values[definitions[1]!.key], '22');
});

test('current values use the assigned contract and newest sample regardless of query order', () => {
  const old = sample(
    'old',
    'old-contract',
    'revision-1',
    1,
    'float64',
    21.5,
    'Temperature',
    'C',
    '2026-01-01T00:00:00Z',
  );
  const earlier = sample(
    'earlier',
    'current-contract',
    'revision-2',
    2,
    'int64',
    '22',
    'Counter',
    'ticks',
    '2026-01-02T00:00:00Z',
  );
  const latest = sample(
    'latest',
    'current-contract',
    'revision-2',
    2,
    'int64',
    '23',
    'Counter',
    'ticks',
    '2026-01-03T00:00:00Z',
  );
  const metrics = [old, earlier, latest];
  assert.deepEqual(currentContractValues(metrics, 'current-contract'), { 'readings./value': '23' });
  assert.equal(latestCurrentMetric(metrics, 'current-contract')?.event_id, 'latest');
  assert.deepEqual(currentContractValues(metrics, 'missing'), {});
});

test('large int64 history stays exact in the table and is omitted from a lossy chart', () => {
  const counter = sample(
    'large',
    'contract',
    'revision',
    1,
    'int64',
    '9007199254740993',
    'Counter',
    'ticks',
    '2026-01-03T00:00:00Z',
  );
  const definition = historyMetricDefinitions([counter])[0]!;
  assert.equal(historyEventRows([counter])[0]?.values[definition.key], '9007199254740993');
  assert.deepEqual(historyChartSamples([counter], definition), []);
  assert.equal(historyHasUnplottableValues([counter], definition), true);
});
