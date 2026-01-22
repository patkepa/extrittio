export interface NavItem {
  label: string;
  icon: string;
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
  icon: string;
}

export interface User {
  name: string;
  email: string;
  avatar?: string;
}
