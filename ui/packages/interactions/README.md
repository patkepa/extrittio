# @extrittio/interactions

Reusable keyboard and focus helpers for dense operational UIs.

## Exports

- `getDirectionalKey`
- `shouldIgnorePageShortcut`
- `isEditableTarget`
- `hasOpenBlockingOverlay`
- `clearKeyboardFocusRegions`
- `moveFocusRegion`
- `useConfirmShortcut`
- `useFormNavigation`
- `useRovingFocus`

## Usage

```tsx
import { useRovingFocus } from "@extrittio/interactions";

const { containerRef } = useRovingFocus({
  itemSelector: '[data-row="true"]',
  enabled: true,
});
```
