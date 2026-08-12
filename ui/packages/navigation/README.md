# @extrittio/navigation

Shared navigation model types used by workspace shells, public site shells, and consuming apps.

## Exports

- `NavItem`
- `NavGroup`
- `WorkspaceNavItem`
- `WorkspaceNavGroup`
- `SiteNavItem`
- `SiteNavGroup`
- `SiteNavAction`
- `Project`
- `User`
- `NavBadge`

## Usage

```ts
import type { NavGroup } from "@extrittio/navigation";

export const navGroups: NavGroup[] = [
  {
    label: "Router",
    items: [{ label: "Interfaces", icon: "exchange", href: "/interfaces" }],
  },
];
```

```ts
import type { SiteNavAction, SiteNavItem } from "@extrittio/navigation";

export const siteNavItems: SiteNavItem[] = [
  { label: "Product", href: "/product" },
  { label: "Blog", href: "/blog" },
];

export const siteActions: SiteNavAction[] = [
  { label: "Request demo", href: "/demo", intent: "primary" },
];
```
