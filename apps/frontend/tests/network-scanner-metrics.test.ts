import assert from 'node:assert/strict';
import test from 'node:test';

import {
  combinedErrorRate,
  energyLevel,
  recommendedChannel,
  signalCondition,
  utilizationCondition,
} from '../src/pages/openthread/network-scanner-metrics.ts';
import type { ThreadChannelDiagnostics } from '../src/types/api.ts';

function channel(
  number: number,
  utilization: number | null,
  energy: number | null,
  networks = 0,
): ThreadChannelDiagnostics {
  return {
    channel: number,
    utilization_percent: utilization,
    max_rssi_dbm: energy,
    network_count: networks,
    strongest_network_rssi_dbm: null,
  };
}

test('classifies channel pressure and received signal independently', () => {
  assert.equal(utilizationCondition(8, 4).label, 'Excellent');
  assert.equal(utilizationCondition(40, 61).label, 'Busy');
  assert.equal(signalCondition(-64).label, 'Good');
  assert.equal(signalCondition(-90).label, 'Weak');
});

test('recommends the quietest channel from utilization, energy, and network count', () => {
  assert.equal(
    recommendedChannel([channel(11, 70, -45, 3), channel(15, 12, -88, 0), channel(20, 18, -72, 1)]),
    15,
  );
  assert.equal(recommendedChannel([channel(11, null, null)]), null);
});

test('normalizes energy and calculates a combined frame error rate', () => {
  assert.equal(energyLevel(-100), 0);
  assert.equal(energyLevel(-30), 100);
  assert.equal(
    combinedErrorRate({
      cca_failure_rate_percent: 0,
      latest_rssi_dbm: -60,
      monitor_sample_count: 50,
      tx_total: 80,
      rx_total: 20,
      tx_retries: 4,
      tx_errors: 2,
      rx_errors: 3,
    }),
    5,
  );
});
