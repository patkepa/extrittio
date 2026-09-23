import assert from 'node:assert/strict';
import test from 'node:test';
import { locatedMapDevices, nextLocationExpiry } from '../src/components/map/map-device-model.ts';
import { loadLocationBatches } from '../src/api/location-batches.ts';

test('map locations come only from matching contract observations, including origin', () => {
  const devices = [{ id: 'device', name: 'Device', status: 'online', last_seen_at: null }];
  const now = Date.parse('2026-09-15T00:00:01Z');
  const location = {
    latitude: 0,
    longitude: 0,
    timestamp: '2026-09-15T00:00:00Z',
    expires_at: '2026-09-15T00:00:05Z',
    contract_id: 'contract',
    event_id: 'event',
  };
  assert.deepEqual(locatedMapDevices(devices, []), []);
  assert.deepEqual(locatedMapDevices(devices, [{ device_id: 'other', location }]), []);
  const result = locatedMapDevices(devices, [{ device_id: 'device', location }], now);
  assert.equal(result.length, 1);
  assert.deepEqual(result[0]?.location, location);
  assert.equal(result[0]?.id, 'device');
  for (const invalid of [
    { latitude: NaN },
    { longitude: Infinity },
    { latitude: 91 },
    { longitude: -181 },
  ]) {
    assert.deepEqual(
      locatedMapDevices(
        devices,
        [{ device_id: 'device', location: { ...location, ...invalid } }],
        now,
      ),
      [],
    );
  }
  assert.equal(nextLocationExpiry([{ location }], now), Date.parse(location.expires_at));
  assert.deepEqual(
    locatedMapDevices(
      devices,
      [{ device_id: 'device', location }],
      Date.parse(location.expires_at),
    ),
    [],
  );
  assert.equal(nextLocationExpiry([{ location }], Date.parse(location.expires_at)), null);
});

test('location batches deduplicate IDs, cap request size and concurrency, and preserve results', async () => {
  const ids = Array.from({ length: 2001 }, (_, index) => `device-${index}`);
  const requests: string[][] = [];
  let active = 0;
  let peak = 0;
  const result = await loadLocationBatches([...ids, ids[0]!], async (batch) => {
    requests.push(batch);
    peak = Math.max(peak, ++active);
    await Promise.resolve();
    active--;
    return batch;
  });
  assert.equal(requests.length, 5);
  assert.ok(requests.every((batch) => batch.length <= 500));
  assert.equal(peak, 3);
  assert.deepEqual(result, [...ids].sort());
  assert.deepEqual(
    await loadLocationBatches([], async () => {
      throw new Error('must not fetch');
    }),
    [],
  );
});

test('a failed location batch rejects the result and stops scheduling further chunks', async () => {
  let calls = 0;
  const ids = Array.from({ length: 2001 }, (_, index) => `device-${index}`);
  await assert.rejects(
    loadLocationBatches(ids, async () => {
      calls++;
      throw new Error('unavailable');
    }),
    /unavailable/,
  );
  assert.ok(calls <= 3);
});
