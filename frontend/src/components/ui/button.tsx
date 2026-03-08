import { forwardRef, type ButtonHTMLAttributes } from 'react';
import { cn } from '@/lib/utils';

type ButtonVariant = 'default' | 'primary' | 'success' | 'danger' | 'warning' | 'ghost' | 'minimal';
type ButtonSize = 'sm' | 'md' | 'lg';

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
}

const variantStyles: Record<ButtonVariant, string> = {
  default: 'bg-surface border border-border text-foreground hover:bg-surface-hover hover:border-border-hover',
  primary: 'bg-accent text-white border border-transparent hover:bg-accent-hover',
  success: 'bg-success text-white border border-transparent hover:brightness-110',
  danger: 'bg-danger text-white border border-transparent hover:brightness-110',
  warning: 'bg-warning text-black border border-transparent hover:brightness-110',
  ghost: 'bg-transparent text-foreground border border-transparent hover:bg-surface',
  minimal: 'bg-transparent text-foreground border border-transparent hover:bg-surface',
};

const sizeStyles: Record<ButtonSize, string> = {
  sm: 'h-7 px-2.5 text-xs gap-1.5',
  md: 'h-8 px-3 text-sm gap-2',
  lg: 'h-10 px-4 text-sm gap-2',
};

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant = 'default', size = 'md', ...props }, ref) => (
    <button
      ref={ref}
      className={cn(
        'inline-flex items-center justify-center rounded-md font-medium transition-all duration-150 cursor-pointer',
        'focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2',
        'disabled:opacity-50 disabled:pointer-events-none',
        variantStyles[variant],
        sizeStyles[size],
        className,
      )}
      {...props}
    />
  ),
);
Button.displayName = 'Button';
