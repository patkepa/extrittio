import assert from 'node:assert/strict';
import test from 'node:test';

import {
  blueprintRuleCommands,
  blueprintRuleMetricFields,
  contractRuleCommands,
  contractRuleMetricFields,
} from '../src/features/rules/model/rule-metric-fields.ts';
import type { DeviceBlueprintRevision, DeviceContract } from '../src/types/api.ts';

test('derives numeric rule fields from a blueprint without fixed sensor names', () => {
  const revision = {
    id: 'revision-a',
    blueprint_id: 'blueprint-a',
    document: {
      spec: {
        streams: [
          {
            key: 'air',
            fields: [
              {
                path: '/particles/pm25',
                type: 'float64',
                label: 'PM2.5',
                semantic: 'particulate_matter_2_5',
              },
              { path: '/state', type: 'string', label: 'State' },
            ],
          },
        ],
        commands: [{ key: 'calibrate', label: 'Calibrate sensor' }],
      },
    },
  } as DeviceBlueprintRevision;

  assert.deepEqual(blueprintRuleMetricFields(revision), [
    {
      value: 'air./particles/pm25',
      label: 'PM2.5 · particulate_matter_2_5',
      blueprint_id: 'blueprint-a',
      blueprint_revision_id: 'revision-a',
    },
  ]);
  assert.deepEqual(blueprintRuleCommands(revision), [
    { value: 'calibrate', label: 'Calibrate sensor' },
  ]);
});

test('derives the same canonical rule field from a compiled device contract', () => {
  const contract = {
    blueprint_revision_id: 'revision-b',
    document: {
      streams: {
        air: {
          fields: {
            '/particles/pm25': {
              valueType: 'float64',
              label: 'PM2.5',
            },
          },
        },
      },
      commands: { calibrate: { label: 'Calibrate sensor' } },
    },
  } as DeviceContract;

  assert.deepEqual(contractRuleMetricFields(contract, 'blueprint-b'), [
    {
      value: 'air./particles/pm25',
      label: 'PM2.5',
      blueprint_id: 'blueprint-b',
      blueprint_revision_id: 'revision-b',
    },
  ]);
  assert.deepEqual(contractRuleCommands(contract), [
    { value: 'calibrate', label: 'Calibrate sensor' },
  ]);
});

test('keeps distinct nested, dotted, and escaped JSON pointer fields', () => {
  const revision = {
    document: {
      spec: {
        streams: [
          {
            key: 'air',
            fields: [
              { path: '/a/b', type: 'int64' },
              { path: '/a.b', type: 'int64' },
              { path: '/a~1b', type: 'int64' },
            ],
          },
        ],
      },
    },
  } as DeviceBlueprintRevision;
  assert.deepEqual(
    blueprintRuleMetricFields(revision).map(({ value }) => value),
    ['air./a/b', 'air./a.b', 'air./a~1b'],
  );
});
