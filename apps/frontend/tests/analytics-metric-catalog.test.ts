import assert from 'node:assert/strict';
import test from 'node:test';

import {
  analyticsMetricId,
  groupAnalyticsMetrics,
} from '../src/features/analytics/model/metric-catalog.ts';

const metrics = [
  {
    blueprint_id: 'cold-room',
    blueprint_key: 'cold-room',
    blueprint_name: 'Cold room',
    stream_key: 'environment',
    field_path: '/temperature',
    key: 'environment./temperature',
    label: 'Temperature',
    unit: 'Cel',
    value_type: 'float64',
    aggregates: ['average'],
    series_modes: ['per_device'],
  },
  {
    blueprint_id: 'power-meter',
    blueprint_key: 'power-meter',
    blueprint_name: 'Power meter',
    stream_key: 'electrical',
    field_path: '/phase/voltage',
    key: 'electrical./phase/voltage',
    label: 'Voltage',
    unit: 'V',
    value_type: 'float64',
    aggregates: ['average'],
    series_modes: ['per_device'],
  },
] as const;

test('uses blueprint, stream, and path as an unambiguous metric identity', () => {
  assert.notEqual(
    analyticsMetricId(metrics[0]),
    analyticsMetricId({ ...metrics[0], blueprint_id: 'another-blueprint' }),
  );
  assert.equal(
    analyticsMetricId(metrics[1]),
    '["power-meter","electrical","/phase/voltage"]',
  );
});

test('groups arbitrary numeric fields by their declaring blueprint', () => {
  const grouped = groupAnalyticsMetrics([...metrics]);
  assert.deepEqual(
    grouped.map((group) => [group.blueprintName, group.metrics[0]?.key]),
    [
      ['Cold room', 'environment./temperature'],
      ['Power meter', 'electrical./phase/voltage'],
    ],
  );
});
