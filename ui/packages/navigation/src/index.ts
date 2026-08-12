import type { IconName } from "@blueprintjs/icons";

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
  environment: string;
  icon: IconName;
  color?: string;
}

export interface User {
  name: string;
  email: string;
  avatar?: string;
}

export interface NavBadge {
  count?: number;
  status?: "online" | "warning" | "offline";
}

export type WorkspaceNavItem = NavItem;
export type WorkspaceNavGroup = NavGroup;

export interface SiteNavItem {
  label: string;
  href: string;
  description?: string;
  external?: boolean;
  children?: SiteNavItem[];
}

export interface SiteNavGroup {
  label: string;
  items: SiteNavItem[];
}

export interface SiteNavAction {
  label: string;
  href: string;
  intent?: "primary" | "secondary";
  external?: boolean;
}
