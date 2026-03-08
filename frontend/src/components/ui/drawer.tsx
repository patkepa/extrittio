import * as DialogPrimitive from '@radix-ui/react-dialog';
import { forwardRef } from 'react';
import { X } from 'lucide-react';
import { cn } from '@/lib/utils';

export const Drawer = DialogPrimitive.Root;
export const DrawerTrigger = DialogPrimitive.Trigger;
export const DrawerClose = DialogPrimitive.Close;

type DrawerSide = 'left' | 'right';

interface DrawerContentProps extends React.ComponentPropsWithoutRef<typeof DialogPrimitive.Content> {
  side?: DrawerSide;
  size?: string;
}

const sideStyles: Record<DrawerSide, string> = {
  right: 'inset-y-0 right-0',
  left: 'inset-y-0 left-0',
};

export const DrawerContent = forwardRef<React.ComponentRef<typeof DialogPrimitive.Content>, DrawerContentProps>(
  ({ className, children, side = 'right', size = '520px', ...props }, ref) => (
    <DialogPrimitive.Portal>
      <DialogPrimitive.Overlay className="fixed inset-0 z-50 bg-black/70" />
      <DialogPrimitive.Content
        ref={ref}
        style={{ width: size }}
        className={cn(
          'fixed z-50 flex flex-col border-l border-border bg-surface shadow-2xl',
          'focus:outline-none',
          sideStyles[side],
          className,
        )}
        {...props}
      >
        {children}
      </DialogPrimitive.Content>
    </DialogPrimitive.Portal>
  ),
);
DrawerContent.displayName = 'DrawerContent';

export function DrawerHeader({ className, children, onClose, ...props }: React.HTMLAttributes<HTMLDivElement> & { onClose?: () => void }) {
  return (
    <div className={cn('flex items-center justify-between px-5 py-4 border-b border-border bg-surface', className)} {...props}>
      <div>{children}</div>
      {onClose && (
        <button onClick={onClose} className="text-muted hover:text-foreground transition-colors">
          <X size={16} />
        </button>
      )}
    </div>
  );
}

export function DrawerBody({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  return <div className={cn('flex-1 overflow-y-auto px-5 py-4', className)} {...props} />;
}

export function DrawerFooter({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  return <div className={cn('px-5 py-3 border-t border-border', className)} {...props} />;
}
