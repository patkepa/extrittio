import { useEffect, type ReactNode } from 'react';
import { Icon } from '@blueprintjs/core';
import { Command } from 'cmdk';
import './command-palette.css';

export interface CommandPaletteShellProps {
  children: ReactNode;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onToggle?: () => void;
  label?: string;
  placeholder?: string;
  emptyText?: string;
}

export const CommandPaletteShell = ({
  children,
  open,
  onOpenChange,
  onToggle,
  label = 'Command Palette',
  placeholder = 'Type a command or search...',
  emptyText = 'No results found.',
}: CommandPaletteShellProps) => {
  useEffect(() => {
    if (!onToggle) return;

    const handler = (event: KeyboardEvent) => {
      if (event.key === 'k' && (event.metaKey || event.ctrlKey)) {
        event.preventDefault();
        onToggle();
      }
    };

    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  }, [onToggle]);

  return (
    <Command.Dialog open={open} onOpenChange={onOpenChange} label={label} loop>
      <div className="cmdk-input-wrapper">
        <Icon icon="search" size={16} />
        <Command.Input placeholder={placeholder} />
        <kbd className="cmdk-kbd">ESC</kbd>
      </div>

      <Command.List>
        <Command.Empty>{emptyText}</Command.Empty>
        {children}
      </Command.List>
    </Command.Dialog>
  );
};
