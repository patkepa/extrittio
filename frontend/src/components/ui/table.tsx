import { forwardRef, type HTMLAttributes, type TdHTMLAttributes, type ThHTMLAttributes } from 'react';
import { cn } from '@/lib/utils';

export const Table = forwardRef<HTMLTableElement, HTMLAttributes<HTMLTableElement>>(
  ({ className, ...props }, ref) => (
    <table ref={ref} className={cn('w-full text-sm text-foreground', className)} {...props} />
  ),
);
Table.displayName = 'Table';

export const TableHead = forwardRef<HTMLTableSectionElement, HTMLAttributes<HTMLTableSectionElement>>(
  ({ className, ...props }, ref) => (
    <thead ref={ref} className={cn('', className)} {...props} />
  ),
);
TableHead.displayName = 'TableHead';

export const TableBody = forwardRef<HTMLTableSectionElement, HTMLAttributes<HTMLTableSectionElement>>(
  ({ className, ...props }, ref) => (
    <tbody ref={ref} className={cn('', className)} {...props} />
  ),
);
TableBody.displayName = 'TableBody';

export const TableRow = forwardRef<HTMLTableRowElement, HTMLAttributes<HTMLTableRowElement>>(
  ({ className, ...props }, ref) => (
    <tr
      ref={ref}
      className={cn('border-t border-border transition-colors hover:bg-surface', className)}
      {...props}
    />
  ),
);
TableRow.displayName = 'TableRow';

export const TableHeader = forwardRef<HTMLTableCellElement, ThHTMLAttributes<HTMLTableCellElement>>(
  ({ className, ...props }, ref) => (
    <th
      ref={ref}
      className={cn(
        'px-3 py-2 text-left text-xs font-semibold uppercase tracking-wider text-muted bg-surface',
        className,
      )}
      {...props}
    />
  ),
);
TableHeader.displayName = 'TableHeader';

export const TableCell = forwardRef<HTMLTableCellElement, TdHTMLAttributes<HTMLTableCellElement>>(
  ({ className, ...props }, ref) => (
    <td ref={ref} className={cn('px-3 py-2.5', className)} {...props} />
  ),
);
TableCell.displayName = 'TableCell';
