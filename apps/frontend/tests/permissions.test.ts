import assert from 'node:assert/strict';
import test from 'node:test';

import { hasPermission, hasRequiredPermissions } from '../src/auth/permissions.ts';

test('management permissions imply their matching read permission', () => {
  assert.equal(hasPermission(['devices.manage'], 'devices.read'), true);
  assert.equal(hasPermission(['rules.manage'], 'rules.read'), true);
});

test('unrelated permissions do not grant access', () => {
  assert.equal(hasPermission(['devices.read'], 'users.read'), false);
  assert.equal(hasRequiredPermissions(undefined, ['devices.read']), false);
});

test('blueprint permissions are independent of retired device-type keys', () => {
  assert.equal(hasPermission(['device_blueprints.manage'], 'device_blueprints.read'), true);
  assert.equal(hasPermission(['device_types.manage'], 'device_blueprints.read'), false);
  assert.equal(hasPermission(['device_types.read'], 'device_blueprints.manage'), false);
});

test('routes without requirements are public to authenticated users', () => {
  assert.equal(hasRequiredPermissions([], undefined), true);
  assert.equal(hasRequiredPermissions([], []), true);
});
