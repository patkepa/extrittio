import {
  LayoutDashboard, Smartphone, Activity, Bell, History,
  Settings, User, Wrench, Palette, MonitorSmartphone,
  Users, Plug, HelpCircle, Code, FlaskConical, Hammer,
} from 'lucide-react';
import type { NavGroup, Project, User as UserType } from '../types/navigation';

export const navGroups: NavGroup[] = [
  {
    label: 'General',
    items: [
      { label: 'Dashboard', icon: LayoutDashboard, href: '/' },
      { label: 'Devices', icon: Smartphone, href: '/devices' },
    ],
  },
  {
    label: 'Analytics',
    items: [
      { label: 'Telemetry', icon: Activity, href: '/telemetry' },
      { label: 'Alerts', icon: Bell, href: '/alerts' },
      { label: 'Audit Log', icon: History, href: '/audit-log' },
    ],
  },
  {
    label: 'Management',
    items: [
      {
        label: 'Settings', icon: Settings, href: '/settings',
        children: [
          { label: 'Profile', icon: User, href: '/settings/profile' },
          { label: 'Account', icon: Wrench, href: '/settings/account' },
          { label: 'Appearance', icon: Palette, href: '/settings/appearance' },
          { label: 'Notifications', icon: Bell, href: '/settings/notifications' },
          { label: 'Display', icon: MonitorSmartphone, href: '/settings/display' },
        ],
      },
      { label: 'Users', icon: Users, href: '/users' },
      { label: 'Integrations', icon: Plug, href: '/integrations' },
      { label: 'Help Center', icon: HelpCircle, href: '/help-center' },
    ],
  },
];

export const projects: Project[] = [
  { name: 'Extrittio', environment: 'Development', icon: Code },
  { name: 'Extrittio', environment: 'Testing', icon: FlaskConical },
  { name: 'Extrittio', environment: 'Production', icon: Hammer },
];

export const currentUser: UserType = {
  name: 'satnaing',
  email: 'satnaingdev@gmail.com',
  avatar: '/avatars/shadcn.jpg',
};
