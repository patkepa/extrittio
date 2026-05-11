# @extrittio/theme

Dark theme provider and CSS variable foundation for Extrittio-style tools.

## Exports

- `ThemeProvider`
- `useTheme`
- `Theme`
- `@extrittio/theme/theme.css`

## Usage

```tsx
import { ThemeProvider } from '@extrittio/theme';
import '@extrittio/theme/theme.css';

export function App() {
  return <ThemeProvider>{/* routes */}</ThemeProvider>;
}
```
