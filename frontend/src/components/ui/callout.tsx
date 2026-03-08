import { type HTMLAttributes, type ReactNode } from 'react';
import { cn } from '@/lib/utils';

type CalloutIntent = 'default' | 'success' | 'warning' | 'danger';

interface CalloutProps extends HTMLAttributes<HTMLDivElement> {
  intent?: CalloutIntent;
  icon?: ReactNode;
  title?: string;
}

const intentStyles: Record<CalloutIntent, string> = {
  default: 'bg-surface border-l-accent',
  success: 'bg-success-muted border-l-success',
  warning: 'bg-warning-muted border-l-warning',
  danger: 'bg-danger-muted border-l-danger',
};

export function Callout({ className, intent = 'default', icon, title, children, ...props }: CalloutProps) {
  return (
    <div
      className={cn(
        'rounded-md border-l-4 p-3',
        intentStyles[intent],
        className,
      )}
      {...props}
    >
      {(icon || title) && (
        <div className="flex items-center gap-2 mb-1">
          {icon}
          {title && <span className="text-sm font-semibold text-foreground">{title}</span>}
        </div>
      )}
      {children && <div className="text-sm text-foreground">{children}</div>}
    </div>
  );
}
