import { NavGroup, Project, User } from '../types/navigation';

export const navGroups: NavGroup[] = [
  {
    label: 'General',
    items: [
      {
        label: 'Dashboard',
        icon: 'dashboard',
        href: '/',
      },
      {
        label: 'Devices',
        icon: 'mobile-video',
        href: '/devices',
      },
      {
        label: 'Fleet Graph',
        icon: 'graph',
        href: '/fleet-graph',
      },
      {
        label: 'Map',
        icon: 'map',
        href: '/map',
      },
    ],
  },
  {
    label: 'Automation',
    items: [
      {
        label: 'Rules',
        icon: 'filter',
        href: '/rules',
      },
      {
        label: 'Alerts',
        icon: 'warning-sign',
        href: '/alerts',
      },
    ],
  },
  {
    label: 'Management',
    items: [
      {
        label: 'Firmware',
        icon: 'upload',
        href: '/settings/firmware',
      },
      {
        label: 'Fleets',
        icon: 'layers',
        href: '/settings/fleets',
      },
      {
        label: 'Settings',
        icon: 'cog',
        href: '/settings',
        children: [
          {
            label: 'Profile',
            icon: 'user',
            href: '/settings/profile',
          },
          {
            label: 'Device Types',
            icon: 'tag',
            href: '/settings/device-types',
          },
          {
            label: 'Users',
            icon: 'people',
            href: '/settings/users',
          },
          { label: 'Certificates', icon: 'lock', href: '/settings/certificates' },
          { label: 'API Keys', icon: 'key', href: '/settings/api-keys' },
        ],
      },
      {
        label: 'Help',
        icon: 'help',
        href: '/help',
      },
    ],
  },
];

export const projects: Project[] = [
  {
    name: 'Extrittio',
    environment: 'Development',
    icon: 'code',
  },
  {
    name: 'Extrittio',
    environment: 'Testing',
    icon: 'lab-test',
  },
  {
    name: 'Extrittio',
    environment: 'Production',
    icon: 'build',
  },
];

export const currentUser: User = {
  name: 'Patryk Kępa',
  email: 'opensource@extrittio.dev',
  avatar: '/avatars/shadcn.jpg',
};
