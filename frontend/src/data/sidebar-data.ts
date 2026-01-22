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
    ],
  },
  {
    label: 'Other',
    items: [
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
            label: 'Account',
            icon: 'wrench',
            href: '/settings/account',
          },
          {
            label: 'Appearance',
            icon: 'style',
            href: '/settings/appearance',
          },
          {
            label: 'Notifications',
            icon: 'notifications',
            href: '/settings/notifications',
          },
          {
            label: 'Display',
            icon: 'desktop',
            href: '/settings/display',
          },
        ],
      },
      {
        label: 'Users',
        icon: 'people',
        href: '/users',
      },
      {
        label: 'Help Center',
        icon: 'help',
        href: '/help-center',
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
  name: 'satnaing',
  email: 'satnaingdev@gmail.com',
  avatar: '/avatars/shadcn.jpg',
};
