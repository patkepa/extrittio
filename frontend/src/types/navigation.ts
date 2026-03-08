import type { IconName } from '@blueprintjs/icons';

export interface NavItem {
  label: string;
  icon: IconName;
  href: string;
  children?: NavItem[];
}

export interface NavGroup {
  label: string;
  items: NavItem[];
}

export interface Project {
  name: string;
  environment: 'Production' | 'Development' | 'Testing';
  icon: IconName;
}

export interface User {
  name: string;
  email: string;
  avatar?: string;
}
