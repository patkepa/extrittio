# @patkepa/theme

Dark theme provider and CSS variable foundation for Extrittio-style tools.

## Exports

- `ThemeProvider`
- `useTheme`
- `Theme`
- `@patkepa/theme/theme.css`

## Usage

```tsx
import { ThemeProvider } from '@patkepa/theme';
import '@patkepa/theme/theme.css';

export function App() {
  return <ThemeProvider>{/* routes */}</ThemeProvider>;
}
```
