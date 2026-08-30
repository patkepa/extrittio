import assert from 'node:assert/strict';
import test from 'node:test';

import {
  alignAnalyticsSeries,
  formatBucket,
  formatMetricValue,
} from '../src/features/analytics/model/chart-data.ts';

test('aligns sparse device series against a shared chronological time axis', () => {
  const data = alignAnalyticsSeries([
    {
      id: 'device:a',
      label: 'A',
      kind: 'device',
      device_id: 'a',
      points: [
        { timestamp_ms: 2_000, value: 22 },
        { timestamp_ms: 4_000, value: 24 },
      ],
      stats: { minimum: 22, maximum: 24, average: 23, latest: 24, sample_count: 2 },
    },
    {
      id: 'device:b',
      label: 'B',
      kind: 'device',
      device_id: 'b',
      points: [{ timestamp_ms: 3_000, value: 18 }],
      stats: { minimum: 18, maximum: 18, average: 18, latest: 18, sample_count: 1 },
    },
  ]);

  assert.deepEqual(data, [
    [2, 3, 4],
    [22, null, 24],
    [null, 18, null],
  ]);
});

test('formats analytics resolution and values for compact UI labels', () => {
  assert.equal(formatBucket(300), '5 min');
  assert.equal(formatBucket(21_600), '6 hr');
  assert.equal(formatMetricValue(21.746, '°C'), '21.7°C');
  assert.equal(formatMetricValue(231.126, 'V', 2), '231.13V');
  assert.equal(formatMetricValue(7, null, 0), '7');
  assert.equal(formatMetricValue(null, '%'), '—');
});
