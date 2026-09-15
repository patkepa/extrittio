import assert from 'node:assert/strict';
import test from 'node:test';
import { QueryClient } from '@tanstack/react-query';
import { queryKeys } from '../src/hooks/query-keys.ts';

test('firmware version invalidation matches every blueprint revision', async () => {
  const client = new QueryClient();
  const first = queryKeys.firmware.nextBlueprintVersion('revision-1');
  const second = queryKeys.firmware.nextBlueprintVersion('revision-2');
  const list = queryKeys.firmware.list();
  client.setQueryData(first, { next_version: '1.0.1' });
  client.setQueryData(second, { next_version: '2.0.1' });
  client.setQueryData(list, []);
  await client.invalidateQueries({ queryKey: queryKeys.firmware.nextBlueprintVersionAll });
  assert.equal(client.getQueryState(first)?.isInvalidated, true);
  assert.equal(client.getQueryState(second)?.isInvalidated, true);
  assert.equal(client.getQueryState(list)?.isInvalidated, false);
  client.clear();
});
