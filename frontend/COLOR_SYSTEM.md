# Blueprint.js Color System Guide

This document explains how colors are properly implemented in the frontend-v2 application using Blueprint.js v6.

## Overview

We use Blueprint.js v6's color system **correctly** by:
1. ✅ Letting Blueprint handle text and UI colors through `.bp5-dark` class
2. ✅ Only overriding structural/background colors via CSS variables
3. ✅ Using Blueprint's `Colors` constant for any custom component styling
4. ✅ Using `intent` props for semantic coloring (primary, success, warning, danger)
5. ❌ **NOT** hardcoding color values or using excessive `!important` flags

## Color System Architecture

### 1. CSS Variables (Custom Theme Colors)

Our custom CSS variables in `src/styles/theme.css` define **structural colors** only:

```css
:root.dark {
  /* Backgrounds */
  --content-bg: hsl(220, 20%, 8%);    /* ~Blueprint DARK_GRAY1 */
  --card-bg: hsl(220, 15%, 11%);      /* ~Blueprint DARK_GRAY2 */
  --navbar-bg: hsl(220, 20%, 8%);     /* ~Blueprint DARK_GRAY1 */

  /* Borders */
  --border-color: hsl(220, 15%, 15%); /* ~Blueprint DARK_GRAY3 */

  /* Accents (mapped to Blueprint colors) */
  --accent: 211 100% 50%;             /* Blueprint BLUE3: #2D72D2 */
}
```

**Note:** These values are intentionally aligned with Blueprint's color scale for consistency.

### 2. Blueprint Colors Constant

For component-level styling, use `src/styles/colors.ts`:

```typescript
import { IntentColors, GrayScale, AndurilTheme } from '@/styles/colors';

// Example usage:
<div style={{ color: AndurilTheme.textPrimary }}>
  <Icon icon="tick" style={{ color: IntentColors.success }} />
</div>
```

Available color groups:
- **IntentColors**: Primary, Success, Warning, Danger (for UI semantics)
- **GrayScale**: Black → White scale (for layout/structure)
- **DataColors**: Extended colors for charts/visualizations
- **AndurilTheme**: Pre-mapped colors matching our Anduril aesthetic

### 3. Blueprint's Intent System

Always use Blueprint's built-in `intent` prop for semantic coloring:

```tsx
// ✅ Correct: Use intent prop
<Button intent="primary">Save</Button>
<Callout intent="danger">Error message</Callout>
<Icon icon="tick-circle" intent="success" />
<Tag intent="warning">Pending</Tag>

// ❌ Incorrect: Don't hardcode colors
<Button style={{ backgroundColor: '#2D72D2' }}>Save</Button>
```

## What Was Fixed

### Before (Problematic):
```css
/* ❌ BAD: Hardcoded Blueprint CSS variables */
.bp5-dark {
  --bp5-text-color: #F6F7F9 !important;
  --bp5-link-color: #4C90F0 !important;
}

/* ❌ BAD: Excessive !important overrides */
.bp5-dark .bp5-button {
  color: #F6F7F9 !important;
}
```

### After (Correct):
```css
/* ✅ GOOD: Let Blueprint handle its own colors */
.bp5-dark {
  background-color: var(--content-bg);
  /* Blueprint handles text colors automatically */
}

/* ✅ GOOD: Minimal, specific overrides without !important */
.bp5-button {
  font-weight: 500;
  /* Let Blueprint's cascade work naturally */
}
```

## Blueprint v6 Color Reference

### Gray Scale (Main UI Frame)
- `Colors.BLACK` - #111418
- `Colors.DARK_GRAY1` - #1C2127
- `Colors.DARK_GRAY2` - #252A31
- `Colors.DARK_GRAY3` - #2F343C
- `Colors.DARK_GRAY4` - #383E47
- `Colors.DARK_GRAY5` - #404854
- `Colors.GRAY1` - #5F6B7C
- `Colors.GRAY2` - #738091
- `Colors.GRAY3` - #8F99A8
- `Colors.GRAY4` - #ABB3BF
- `Colors.GRAY5` - #C5CBD3
- `Colors.LIGHT_GRAY1` - #D3D8DE
- `Colors.LIGHT_GRAY2` - #DCE0E5
- `Colors.LIGHT_GRAY3` - #E5E8EB
- `Colors.LIGHT_GRAY4` - #EDEFF2
- `Colors.LIGHT_GRAY5` - #F6F7F9
- `Colors.WHITE` - #FFFFFF

### Core Colors (Intent-Based)
- **Blue (Primary)**: `Colors.BLUE1` through `Colors.BLUE5`
  - BLUE3: #2D72D2 (Default primary)
- **Green (Success)**: `Colors.GREEN1` through `Colors.GREEN5`
  - GREEN3: #238551 (Default success)
- **Orange (Warning)**: `Colors.ORANGE1` through `Colors.ORANGE5`
  - ORANGE3: #C87619 (Default warning)
- **Red (Danger)**: `Colors.RED1` through `Colors.RED5`
  - RED3: #CD4246 (Default danger)

## Best Practices

### ✅ DO:
- Use Blueprint's `intent` prop for semantic colors
- Import from `@/styles/colors.ts` for custom styling
- Reference CSS variables for structural colors (backgrounds, borders)
- Let Blueprint's `.bp5-dark` class handle text colors
- Use Blueprint components as designed

### ❌ DON'T:
- Hardcode hex color values in components
- Override Blueprint CSS variables with hardcoded values
- Use `!important` flags (except for very specific edge cases)
- Fight against Blueprint's color cascade
- Mix color systems (stick to Blueprint's)

## Example Components

### Using Intent Props (Preferred)
```tsx
import { Button, Callout, Tag } from '@blueprintjs/core';

export function MyComponent() {
  return (
    <>
      <Button intent="primary" icon="add">Add Device</Button>
      <Callout intent="warning">Check your settings</Callout>
      <Tag intent="success">Active</Tag>
    </>
  );
}
```

### Using Colors Constant (When Needed)
```tsx
import { IntentColors } from '@/styles/colors';

export function CustomBadge({ status }: { status: string }) {
  const color = status === 'online'
    ? IntentColors.success
    : IntentColors.danger;

  return <span style={{ color }}>{status}</span>;
}
```

### Using CSS Variables (For Layout)
```css
.custom-panel {
  background-color: var(--card-bg);
  border: 1px solid var(--border-color);
  /* Blueprint handles text color via .bp5-dark */
}
```

## Resources

- [Blueprint v6 Colors Documentation](https://blueprintjs.com/docs/#core/colors)
- [Blueprint v6 Dark Theme](https://blueprintjs.com/docs/#core/colors.dark-theme)
- [Blueprint Colors API](https://blueprintjs.com/docs/#core/colors.colors-constant)

## Troubleshooting

**Q: Text is not visible in dark theme**
- ✅ Ensure `<html class="bp5-dark">` is set
- ✅ Don't override Blueprint's text color variables
- ✅ Let Blueprint handle text colors automatically

**Q: Colors look wrong**
- ✅ Check you're using `intent` props correctly
- ✅ Verify no hardcoded colors override Blueprint
- ✅ Remove any `!important` flags

**Q: Need a custom color?**
- ✅ Import from `@/styles/colors.ts`
- ✅ Use inline styles or CSS variables
- ✅ Don't hardcode hex values directly
