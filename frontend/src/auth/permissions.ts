export type PermissionKey =
  | 'api_keys.manage'
  | 'alerts.manage'
  | 'alerts.read'
  | 'commands.read'
  | 'commands.send'
  | 'device_types.manage'
  | 'device_types.read'
  | 'devices.manage'
  | 'devices.read'
  | 'firmware.deploy'
  | 'firmware.manage'
  | 'firmware.read'
  | 'fleets.manage'
  | 'fleets.read'
  | 'logs.read'
  | 'roles.manage'
  | 'roles.read'
  | 'rules.manage'
  | 'rules.read'
  | 'server_metrics.read'
  | 'shadows.manage'
  | 'shadows.read'
  | 'telemetry.read'
  | 'users.manage'
  | 'users.read'
  | 'zones.manage'
  | 'zones.read';

export function hasPermission(
  permissions: readonly string[] | undefined,
  permission: PermissionKey,
) {
  if (!permissions) return false;
  if (permissions.includes(permission)) return true;

  const alternatives = IMPLIED_PERMISSIONS[permission] ?? [];
  return alternatives.some((alternative) => permissions.includes(alternative));
}

export function hasRequiredPermissions(
  permissions: readonly string[] | undefined,
  requiredPermissions?: readonly PermissionKey[],
) {
  if (!requiredPermissions || requiredPermissions.length === 0) return true;
  return requiredPermissions.every((permission) => hasPermission(permissions, permission));
}

const IMPLIED_PERMISSIONS: Partial<Record<PermissionKey, PermissionKey[]>> = {
  'alerts.read': ['alerts.manage'],
  'commands.read': ['commands.send'],
  'device_types.read': ['device_types.manage'],
  'devices.read': ['devices.manage'],
  'firmware.read': ['firmware.manage', 'firmware.deploy'],
  'fleets.read': ['fleets.manage'],
  'roles.read': ['roles.manage'],
  'rules.read': ['rules.manage'],
  'shadows.read': ['shadows.manage'],
  'users.read': ['users.manage'],
  'zones.read': ['zones.manage'],
};
