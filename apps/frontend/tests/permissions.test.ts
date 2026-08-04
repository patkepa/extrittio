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

test('routes without requirements are public to authenticated users', () => {
  assert.equal(hasRequiredPermissions([], undefined), true);
  assert.equal(hasRequiredPermissions([], []), true);
});
