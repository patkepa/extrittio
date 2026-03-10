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

