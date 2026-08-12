# @extrittio/app-shell

Reusable layout chrome for Extrittio applications.

- `Workspace*` components are for the authenticated SaaS workspace where users do their primary work.
- `Site*` components are for public-facing pages such as the product site, blog, gallery, docs landing pages, and marketing content.

`AppShell` and `AppSidebar` remain available as backward-compatible aliases for the workspace components.

## Exports

- `WorkspaceShell`
- `WorkspaceNavbar`
- `WorkspaceSidebar`
- `SiteShell`
- `SiteNavbar`
- `SiteFooter`
- `AppShell`
- `AppSidebar`
- `ErrorBoundary`
- `WorkspaceShellProps`
- `WorkspaceNavbarProps`
- `WorkspaceSidebarProps`
- `SiteShellProps`
- `SiteNavbarProps`
- `SiteFooterProps`
- `AppShellProps`
- `AppSidebarProps`
- `ErrorBoundaryProps`
- `@extrittio/app-shell/styles.css`

## Workspace Usage

```tsx
import { WorkspaceShell } from "@extrittio/app-shell";
import type { NavGroup } from "@extrittio/navigation";

import "@extrittio/app-shell/styles.css";

const navGroups: NavGroup[] = [
  {
    label: "Operations",
    items: [{ label: "Overview", icon: "dashboard", href: "/" }],
  },
];

export function ToolShell({ children }: { children: React.ReactNode }) {
  return (
    <WorkspaceShell
      productName="Router Workspace"
      navGroups={navGroups}
      sidebarCollapsed={false}
      onToggleSidebar={() => undefined}
    >
      {children}
    </WorkspaceShell>
  );
}
```

Keep product-specific route data, auth stores, breadcrumbs, and command palettes in an app adapter.

## Site Usage

```tsx
import { SiteFooter, SiteShell } from "@extrittio/app-shell";
import type { SiteNavAction, SiteNavItem } from "@extrittio/navigation";

import "@extrittio/app-shell/styles.css";

const navItems: SiteNavItem[] = [
  { label: "Product", href: "/product" },
  { label: "Blog", href: "/blog" },
  { label: "Gallery", href: "/gallery" },
];

const actions: SiteNavAction[] = [
  { label: "Sign in", href: "/login" },
  { label: "Request demo", href: "/demo", intent: "primary" },
];

export function PublicSite({ children }: { children: React.ReactNode }) {
  return (
    <SiteShell
      productName="Extrittio"
      navItems={navItems}
      actions={actions}
      footer={<SiteFooter productName="Extrittio" />}
    >
      {children}
    </SiteShell>
  );
}
```
