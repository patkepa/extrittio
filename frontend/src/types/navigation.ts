import type { LucideIcon } from 'lucide-react';

export interface NavItem {
  label: string;
  icon: LucideIcon;
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
  icon: LucideIcon;
}

export interface User {
  name: string;
  email: string;
  avatar?: string;
}
