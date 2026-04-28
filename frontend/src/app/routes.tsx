import { lazy, type ReactNode } from 'react';
import type { IconName } from '@blueprintjs/icons';
import type { NavGroup, NavItem } from '../types/navigation';

const Dashboard = lazy(() => import('../pages/dashboard').then((m) => ({ default: m.Dashboard })));
const Devices = lazy(() => import('../pages/devices').then((m) => ({ default: m.Devices })));
const DeviceDetail = lazy(() =>
  import('../pages/device-detail').then((m) => ({ default: m.DeviceDetail })),
);
const FleetGraph = lazy(() =>
  import('../pages/fleet-graph').then((m) => ({ default: m.FleetGraph })),
);
const Settings = lazy(() =>
  import('../pages/settings/index').then((m) => ({ default: m.Settings })),
);
const Help = lazy(() => import('../pages/help').then((m) => ({ default: m.Help })));
const Rules = lazy(() => import('../pages/rules').then((m) => ({ default: m.Rules })));
const Alerts = lazy(() => import('../pages/alerts').then((m) => ({ default: m.Alerts })));
const MapPage = lazy(() => import('../pages/map-page'));

export interface AppRoute {
  id: string;
  label: string;
  path: string;
  href?: string;
  icon: IconName;
  element?: ReactNode;
  navGroup?: 'General' | 'Automation' | 'Management';
  children?: AppRoute[];
  showInCommandPalette?: boolean;
}

export interface SettingsRoute {
  id: string;
  label: string;
  path: string;
  icon: IconName;
}

export const settingsRoutes: SettingsRoute[] = [
  { id: 'profile', label: 'Profile', icon: 'user', path: 'profile' },
  { id: 'device-types', label: 'Device Types', icon: 'tag', path: 'device-types' },
  { id: 'fleets', label: 'Fleets', icon: 'layers', path: 'fleets' },
  { id: 'users', label: 'Users', icon: 'people', path: 'users' },
  { id: 'firmware', label: 'Firmware', icon: 'upload', path: 'firmware' },
  { id: 'certificates', label: 'Certificates', icon: 'lock', path: 'certificates' },
  { id: 'api-keys', label: 'API Keys', icon: 'key', path: 'api-keys' },
];

export const appRoutes: AppRoute[] = [
  {
    id: 'dashboard',
    label: 'Dashboard',
    icon: 'dashboard',
    path: '/',
    element: <Dashboard />,
    navGroup: 'General',
    showInCommandPalette: true,
  },
  {
    id: 'devices',
    label: 'Devices',
    icon: 'mobile-video',
    path: '/devices',
    element: <Devices />,
    navGroup: 'General',
    showInCommandPalette: true,
  },
  {
    id: 'device-detail',
    label: 'Device Detail',
    icon: 'mobile-video',
    path: '/devices/:deviceId',
    element: <DeviceDetail />,
  },
  {
    id: 'fleet-graph',
    label: 'Fleet Graph',
    icon: 'graph',
    path: '/fleet-graph',
    element: <FleetGraph />,
    navGroup: 'General',
    showInCommandPalette: true,
  },
  {
    id: 'map',
    label: 'Map',
    icon: 'map',
    path: '/map',
    element: <MapPage />,
    navGroup: 'General',
    showInCommandPalette: true,
  },
  {
    id: 'rules',
    label: 'Rules',
    icon: 'filter',
    path: '/rules',
    element: <Rules />,
    navGroup: 'Automation',
    showInCommandPalette: true,
  },
  {
    id: 'alerts',
    label: 'Alerts',
    icon: 'warning-sign',
    path: '/alerts',
    element: <Alerts />,
    navGroup: 'Automation',
    showInCommandPalette: true,
  },
  {
    id: 'firmware',
    label: 'Firmware',
    icon: 'upload',
    path: '/settings/firmware',
    navGroup: 'Management',
    showInCommandPalette: true,
  },
  {
    id: 'fleets',
    label: 'Fleets',
    icon: 'layers',
    path: '/settings/fleets',
    navGroup: 'Management',
    showInCommandPalette: true,
  },
  {
    id: 'settings',
    label: 'Settings',
    icon: 'cog',
    path: '/settings/*',
    href: '/settings',
    element: <Settings />,
    navGroup: 'Management',
    showInCommandPalette: true,
    children: settingsRoutes.map((route) => ({
      id: `settings-${route.id}`,
      label: route.label,
      icon: route.icon,
      path: `/settings/${route.path}`,
      showInCommandPalette: true,
    })),
  },
  {
    id: 'help',
    label: 'Help',
    icon: 'help',
    path: '/help',
    element: <Help />,
    navGroup: 'Management',
    showInCommandPalette: true,
  },
];

export const protectedRoutes = appRoutes.filter((route) => route.element);

function toNavItem(route: AppRoute): NavItem {
  return {
    label: route.label,
    icon: route.icon,
    href: route.href ?? route.path,
    children: route.children?.map(toNavItem),
  };
}

export const navGroups: NavGroup[] = (['General', 'Automation', 'Management'] as const).map(
  (group) => ({
    label: group,
    items: appRoutes.filter((route) => route.navGroup === group).map(toNavItem),
  }),
);

export const commandPaletteRoutes = appRoutes.flatMap((route) => {
  const entries: AppRoute[] = route.showInCommandPalette ? [route] : [];
  return route.children ? entries.concat(route.children) : entries;
});

export function getRouteLabel(pathname: string): string {
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
