import { type HTMLAttributes } from 'react';
import { cn } from '@/lib/utils';

type BadgeVariant = 'default' | 'accent' | 'success' | 'warning' | 'danger';

interface BadgeProps extends HTMLAttributes<HTMLSpanElement> {
  variant?: BadgeVariant;
  round?: boolean;
}

const variantStyles: Record<BadgeVariant, string> = {
  default: 'bg-surface border-border text-foreground',
  accent: 'bg-accent-muted border-transparent text-accent',
  success: 'bg-success-muted border-transparent text-success',
  warning: 'bg-warning-muted border-transparent text-warning',
  danger: 'bg-danger-muted border-transparent text-danger',
};

export function Badge({ className, variant = 'default', round = false, ...props }: BadgeProps) {
  return (
    <span
      className={cn(
        'inline-flex items-center justify-center border px-1.5 text-[10px] font-medium tabular-nums',
        round ? 'rounded-full' : 'rounded',
        variantStyles[variant],
        className,
      )}
      {...props}
    />
  );
}
