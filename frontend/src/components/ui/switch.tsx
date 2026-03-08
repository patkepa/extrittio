import * as SwitchPrimitive from '@radix-ui/react-switch';
import { forwardRef } from 'react';
import { cn } from '@/lib/utils';

interface SwitchProps extends React.ComponentPropsWithoutRef<typeof SwitchPrimitive.Root> {
  label?: string;
}

export const Switch = forwardRef<React.ComponentRef<typeof SwitchPrimitive.Root>, SwitchProps>(
  ({ className, label, ...props }, ref) => (
    <label className="flex items-center gap-2.5 cursor-pointer">
      <SwitchPrimitive.Root
        ref={ref}
        className={cn(
          'relative h-5 w-9 shrink-0 rounded-full border border-border bg-surface transition-colors',
          'data-[state=checked]:bg-accent data-[state=checked]:border-accent',
          'focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2',
          'disabled:opacity-50 disabled:cursor-not-allowed',
          className,
        )}
        {...props}
      >
        <SwitchPrimitive.Thumb
          className={cn(
            'block h-4 w-4 rounded-full bg-foreground shadow transition-transform',
            'data-[state=checked]:translate-x-4 data-[state=unchecked]:translate-x-0.5',
          )}
        />
      </SwitchPrimitive.Root>
      {label && <span className="text-sm font-medium text-foreground">{label}</span>}
    </label>
  ),
);
Switch.displayName = 'Switch';
