import { lazy, type ReactNode } from 'react';
import type { IconName } from '@blueprintjs/icons';
import { hasRequiredPermissions, type PermissionKey } from '../auth/permissions';
import type { NavGroup, NavItem } from '../types/navigation';

const Dashboard = lazy(() => import('../pages/dashboard').then((m) => ({ default: m.Dashboard })));
const Analytics = lazy(() =>
  import('../features/analytics').then((m) => ({ default: m.Analytics })),
);
const Devices = lazy(() => import('../features/devices').then((m) => ({ default: m.Devices })));
const Updates = lazy(() => import('../pages/updates').then((m) => ({ default: m.Updates })));
const DeviceDetail = lazy(() =>
  import('../features/devices').then((m) => ({ default: m.DeviceDetail })),
);
const FleetGraph = lazy(() =>
  import('../pages/fleet-graph').then((m) => ({ default: m.FleetGraph })),
);
const OpenThread = lazy(() =>
  import('../pages/openthread').then((m) => ({ default: m.OpenThread })),
);
const Settings = lazy(() =>
  import('../pages/settings/index').then((m) => ({ default: m.Settings })),
);
const Help = lazy(() => import('../pages/help').then((m) => ({ default: m.Help })));
const Rules = lazy(() => import('../features/rules').then((m) => ({ default: m.Rules })));
const Alerts = lazy(() => import('../features/alerts').then((m) => ({ default: m.Alerts })));
const ActivityLogs = lazy(() =>
  import('../features/activity').then((m) => ({ default: m.ActivityLogs })),
);
const MapPage = lazy(() => import('../pages/map-page'));

export interface AppRoute {
  id: string;
  label: string;
  path: string;
  href?: string;
  icon: IconName;
  element?: ReactNode;
  children?: AppRoute[];
  showInCommandPalette?: boolean;
  requiredPermissions?: PermissionKey[];
}

export interface SettingsRoute {
  id: string;
  label: string;
  path: string;
  icon: IconName;
  requiredPermissions?: PermissionKey[];
}

export const settingsRoutes: SettingsRoute[] = [
  { id: 'profile', label: 'Profile', icon: 'user', path: 'profile' },
  {
    id: 'device-blueprints',
    label: 'Device Blueprints',
    icon: 'diagram-tree',
    path: 'device-blueprints',
    requiredPermissions: ['device_blueprints.read'],
  },
  {
    id: 'fleets',
    label: 'Fleets',
    icon: 'layers',
    path: 'fleets',
    requiredPermissions: ['fleets.read'],
  },
  {
    id: 'users',
    label: 'Users',
    icon: 'people',
    path: 'users',
    requiredPermissions: ['users.read'],
  },
  {
    id: 'roles',
    label: 'Roles',
    icon: 'shield',
    path: 'roles',
    requiredPermissions: ['roles.read'],
  },
  {
    id: 'firmware',
    label: 'Firmware',
    icon: 'upload',
    path: 'firmware',
    requiredPermissions: ['firmware.read'],
  },
  {
    id: 'certificates',
    label: 'Certificates',
    icon: 'lock',
    path: 'certificates',
    requiredPermissions: ['devices.read'],
  },
  {
    id: 'api-keys',
    label: 'API Keys',
    icon: 'key',
    path: 'api-keys',
    requiredPermissions: ['api_keys.manage'],
  },
];

export const appRoutes: AppRoute[] = [
  {
    id: 'dashboard',
    label: 'Dashboard',
    icon: 'dashboard',
    path: '/',
    element: <Dashboard />,
    showInCommandPalette: true,
    requiredPermissions: ['devices.read'],
  },
  {
    id: 'devices',
    label: 'Devices',
    icon: 'mobile-video',
    path: '/devices',
    element: <Devices />,
    showInCommandPalette: true,
    requiredPermissions: ['devices.read'],
  },
  {
    id: 'analytics',
    label: 'Analytics',
    icon: 'timeline-line-chart',
    path: '/analytics',
    element: <Analytics />,
    showInCommandPalette: true,
    requiredPermissions: ['telemetry.read', 'devices.read', 'fleets.read'],
  },
  {
    id: 'device-detail',
    label: 'Device Detail',
    icon: 'mobile-video',
    path: '/devices/:deviceId',
    element: <DeviceDetail />,
    requiredPermissions: ['devices.read'],
  },
  {
    id: 'fleet-graph',
    label: 'Fleet Graph',
    icon: 'graph',
    path: '/fleet-graph',
    element: <FleetGraph />,
    showInCommandPalette: true,
    requiredPermissions: ['devices.read', 'fleets.read'],
  },
  {
    id: 'map',
    label: 'Map',
    icon: 'map',
    path: '/map',
    element: <MapPage />,
    showInCommandPalette: true,
    requiredPermissions: ['devices.read', 'telemetry.read', 'zones.read'],
  },
  {
    id: 'openthread-router',
    label: 'Mesh Network',
    icon: 'satellite',
    path: '/openthread/*',
    href: '/openthread/scanner',
    element: <OpenThread />,
    requiredPermissions: ['roles.manage'],
  },
  {
    id: 'openthread-scanner',
    label: 'Network Scanner',
    icon: 'signal-search',
    path: '/openthread/scanner',
    showInCommandPalette: true,
    requiredPermissions: ['roles.manage'],
  },
  {
    id: 'openthread-mesh',
    label: 'OpenThread Mesh',
    icon: 'graph',
    path: '/openthread/mesh',
    showInCommandPalette: true,
    requiredPermissions: ['roles.manage'],
  },
  {
    id: 'openthread-settings',
    label: 'OpenThread Settings',
    icon: 'cog',
    path: '/openthread/settings',
    showInCommandPalette: true,
    requiredPermissions: ['roles.manage'],
  },
  {
    id: 'updates',
    label: 'Updates',
    icon: 'updated',
    path: '/updates',
    element: <Updates />,
    showInCommandPalette: true,
    requiredPermissions: ['firmware.read'],
  },
  {
    id: 'rules',
    label: 'Rules',
    icon: 'filter',
    path: '/rules',
    element: <Rules />,
    showInCommandPalette: true,
    requiredPermissions: ['rules.read'],
  },
  {
    id: 'alerts',
    label: 'Alerts',
    icon: 'warning-sign',
    path: '/alerts',
    element: <Alerts />,
    showInCommandPalette: true,
    requiredPermissions: ['alerts.read'],
  },
  {
    id: 'firmware',
    label: 'Firmware',
    icon: 'upload',
    path: '/settings/firmware',
    showInCommandPalette: true,
    requiredPermissions: ['firmware.read'],
  },
  {
    id: 'fleets',
    label: 'Fleets',
    icon: 'layers',
    path: '/settings/fleets',
    showInCommandPalette: true,
    requiredPermissions: ['fleets.read'],
  },
  {
    id: 'logs',
    label: 'Logs',
    icon: 'document-open',
    path: '/logs',
    element: <ActivityLogs />,
    showInCommandPalette: true,
    requiredPermissions: ['logs.read'],
  },
  {
    id: 'help',
    label: 'Help',
    icon: 'help',
    path: '/help',
    element: <Help />,
    showInCommandPalette: true,
  },
  {
    id: 'settings',
    label: 'Settings',
    icon: 'cog',
    path: '/settings/*',
    href: '/settings',
    element: <Settings />,
    showInCommandPalette: true,
    children: settingsRoutes.map((route) => ({
      id: `settings-${route.id}`,
      label: route.label,
      icon: route.icon,
      path: `/settings/${route.path}`,
      showInCommandPalette: true,
      requiredPermissions: route.requiredPermissions,
    })),
  },
];

export const protectedRoutes = appRoutes.filter((route) => route.element);

export function canAccessRoute(
  route: Pick<AppRoute, 'requiredPermissions'>,
  permissions: readonly string[] | undefined,
): boolean {
  return hasRequiredPermissions(permissions, route.requiredPermissions);
}

export function canAccessSettingsRoute(
  route: Pick<SettingsRoute, 'requiredPermissions'>,
  permissions: readonly string[] | undefined,
): boolean {
  return hasRequiredPermissions(permissions, route.requiredPermissions);
}

export function getAccessibleSettingsRoutes(permissions: readonly string[] | undefined) {
  return settingsRoutes.filter((route) => canAccessSettingsRoute(route, permissions));
}

export function getDefaultSettingsPath(permissions: readonly string[] | undefined) {
  return (
    getAccessibleSettingsRoutes(permissions).find((route) => route.id !== 'profile')?.path ??
    'profile'
  );
}

function withAccessibleChildren(route: AppRoute, permissions: readonly string[] | undefined) {
  const children = route.children?.filter((child) => canAccessRoute(child, permissions));
  return { ...route, children };
}

export function getAccessibleAppRoutes(permissions: readonly string[] | undefined): AppRoute[] {
  return appRoutes
    .filter((route) => canAccessRoute(route, permissions))
    .map((route) => withAccessibleChildren(route, permissions));
}

export function getAccessibleProtectedRoutes(permissions: readonly string[] | undefined) {
  return protectedRoutes.filter((route) => canAccessRoute(route, permissions));
}

function toNavItem(route: AppRoute): NavItem {
  return {
    label: route.label,
    icon: route.icon,
    href: route.href ?? route.path,
    children: route.children?.map(toNavItem),
  };
}

export function getNavGroups(permissions: readonly string[] | undefined): NavGroup[] {
  const routes = getAccessibleAppRoutes(permissions);
  const findRoute = (id: string) => routes.find((route) => route.id === id);
  const deviceViews = ['devices', 'map', 'fleet-graph']
    .map(findRoute)
    .filter((route): route is AppRoute => Boolean(route))
    .map((route) => ({
      ...toNavItem(route),
      label: route.id === 'devices' ? 'List' : route.id === 'fleet-graph' ? 'Graph' : 'Map',
    }));

  const monitor = ['dashboard', 'devices', 'alerts', 'analytics']
    .map(findRoute)
    .filter((route): route is AppRoute => Boolean(route))
    .map((route) =>
      route.id === 'devices' && deviceViews.length > 1
        ? { ...toNavItem(route), children: deviceViews }
        : toNavItem(route),
    );

  const section = (label: string, icon: IconName, ids: string[]): NavItem | null => {
    const items = ids
      .map(findRoute)
      .filter((route): route is AppRoute => Boolean(route))
      .map(toNavItem);
    const first = items[0];
    if (!first) return null;
    if (items.length === 1) return first;
    return { label, icon, href: first.href, children: items };
  };

  const settings = findRoute('settings');
  const settingsChildren = settings?.children?.filter((child) => child.id !== 'settings-profile');
  const more = [
    section('Operate', 'filter', ['rules', 'updates', 'logs']),
    section('Network', 'satellite', [
      'openthread-scanner',
      'openthread-mesh',
      'openthread-settings',
    ]),
    settings && settingsChildren?.length
      ? toNavItem({ ...settings, children: settingsChildren })
      : null,
  ].filter((item): item is NavItem => Boolean(item));

  return [
    ...(monitor.length ? [{ label: 'Monitor', items: monitor }] : []),
    ...(more.length ? [{ label: 'More', items: more }] : []),
  ];
}

export function getCommandPaletteRoutes(permissions: readonly string[] | undefined) {
  return getAccessibleAppRoutes(permissions).flatMap((route) => {
    const entries: AppRoute[] = route.showInCommandPalette ? [route] : [];
    return route.children ? entries.concat(route.children) : entries;
  });
}

export function getDefaultRoutePath(permissions: readonly string[] | undefined): string {
  for (const route of getAccessibleAppRoutes(permissions)) {
    if (route.path === '/settings/*') {
      return route.children?.[0]?.path ?? '/settings/profile';
    }

    if (!route.element || route.path.includes(':')) continue;
    return route.href ?? route.path;
  }

  return '/settings/profile';
}

export function getRouteLabel(pathname: string, permissions?: readonly string[]): string {
  if (pathname === '/map') return 'Devices / Map';
  if (pathname === '/fleet-graph') return 'Devices / Graph';

  const commandPaletteRoutes = getCommandPaletteRoutes(permissions);
  const exact = commandPaletteRoutes.find((route) => route.path === pathname);
  if (exact) return exact.label;

  if (pathname.startsWith('/settings/')) {
    const child = commandPaletteRoutes.find((route) => route.path === pathname);
    return child ? `Settings / ${child.label}` : 'Settings';
  }

  return pathname
    .split('/')
    .filter(Boolean)
    .map((segment) => segment.charAt(0).toUpperCase() + segment.slice(1))
    .join(' / ');
}
