# @extrittio/ui

Reusable UI primitives shared by the Extrittio app and future internal tools.

## Exports

- `MainToolbar`
- `BottomToolbar`
- `StatusLed`
- `SearchField`
- `FilterPill`
- `EmptyState`
- `@extrittio/ui/styles.css`

## Usage

```tsx
import { EmptyState, SearchField, StatusLed } from '@extrittio/ui';
import '@extrittio/ui/styles.css';

export function DeviceSearch() {
  return (
    <>
      <SearchField value="" onChange={() => undefined} placeholder="Search devices" />
      <StatusLed status="online" label="Online" />
      <EmptyState icon="search" title="No devices found" />
    </>
  );
}
```
