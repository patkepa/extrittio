# @patkepa/app-shell

Reusable application chrome for operational tools: sidebar, top navigation, breadcrumbs, keyboard shell shortcuts, and an error boundary.

## Exports

- `AppShell`
- `AppSidebar`
- `ErrorBoundary`
- `AppShellProps`
- `AppSidebarProps`
- `ErrorBoundaryProps`
- `@patkepa/app-shell/styles.css`

## Usage

```tsx
import { AppShell } from '@patkepa/app-shell';
import type { NavGroup } from '@patkepa/navigation';

import '@patkepa/app-shell/styles.css';

const navGroups: NavGroup[] = [
  {
    label: 'Operations',
    items: [{ label: 'Overview', icon: 'dashboard', href: '/' }],
  },
];

export function ToolShell({ children }: { children: React.ReactNode }) {
  return (
    <AppShell
      productName="Router Admin"
      navGroups={navGroups}
      sidebarCollapsed={false}
      onToggleSidebar={() => undefined}
    >
      {children}
    </AppShell>
  );
}
```

Keep product-specific route data, auth stores, breadcrumbs, and command palettes in an app adapter.
