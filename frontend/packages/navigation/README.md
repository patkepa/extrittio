# @patkepa/navigation

Shared navigation model types used by the app shell and consuming apps.

## Exports

- `NavItem`
- `NavGroup`
- `Project`
- `User`
- `NavBadge`

## Usage

```ts
import type { NavGroup } from '@patkepa/navigation';

export const navGroups: NavGroup[] = [
  {
    label: 'Router',
    items: [{ label: 'Interfaces', icon: 'exchange', href: '/interfaces' }],
  },
];
```
