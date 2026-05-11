# @patkepa/command-palette

Reusable command palette shell built on `cmdk`. Apps provide the command groups and command items.

## Exports

- `CommandPaletteShell`
- `CommandPaletteShellProps`
- `@patkepa/command-palette/styles.css`

## Usage

```tsx
import { Command } from 'cmdk';
import { CommandPaletteShell } from '@patkepa/command-palette';

export function Palette({ open, setOpen }: { open: boolean; setOpen: (open: boolean) => void }) {
  return (
    <CommandPaletteShell open={open} onOpenChange={setOpen} onToggle={() => setOpen(!open)}>
      <Command.Group heading="Navigation">
        <Command.Item onSelect={() => setOpen(false)}>Dashboard</Command.Item>
      </Command.Group>
    </CommandPaletteShell>
  );
}
```
