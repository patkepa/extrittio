import { forwardRef, type InputHTMLAttributes, type ReactNode } from 'react';
import { cn } from '@/lib/utils';

interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  leftIcon?: ReactNode;
  rightElement?: ReactNode;
}

export const Input = forwardRef<HTMLInputElement, InputProps>(
  ({ className, leftIcon, rightElement, ...props }, ref) => {
    if (leftIcon || rightElement) {
      return (
        <div className="relative flex items-center">
          {leftIcon && (
            <div className="absolute left-2.5 text-muted pointer-events-none">{leftIcon}</div>
          )}
          <input
            ref={ref}
            className={cn(
              'h-8 w-full rounded-md border border-border bg-surface px-3 text-sm text-foreground',
              'placeholder:text-muted',
              'focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent',
              'transition-colors duration-150',
              leftIcon && 'pl-8',
              rightElement && 'pr-8',
              className,
            )}
            {...props}
          />
          {rightElement && (
            <div className="absolute right-1.5">{rightElement}</div>
          )}
        </div>
      );
    }

    return (
      <input
        ref={ref}
        className={cn(
          'h-8 w-full rounded-md border border-border bg-surface px-3 text-sm text-foreground',
          'placeholder:text-muted',
          'focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent',
          'transition-colors duration-150',
          className,
        )}
        {...props}
      />
    );
  },
);
Input.displayName = 'Input';
