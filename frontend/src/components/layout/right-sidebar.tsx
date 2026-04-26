import type { CSSProperties, ReactNode } from 'react';
import './right-sidebar.css';

interface RightSidebarProps {
  children: ReactNode;
  collapsed?: boolean;
  className?: string;
  mode?: 'inline' | 'overlay';
  width?: number | string;
  ariaLabel?: string;
}

export const RightSidebar = ({
  children,
  collapsed = false,
  className,
  mode = 'inline',
  width,
  ariaLabel,
}: RightSidebarProps) => {
  const style =
    width === undefined
      ? undefined
      : ({
          '--right-sidebar-width': typeof width === 'number' ? `${width}px` : width,
        } as CSSProperties);

  return (
    <aside
      className={[
        'right-sidebar',
        `right-sidebar--${mode}`,
        collapsed && 'right-sidebar--collapsed',
        className,
      ]
        .filter(Boolean)
        .join(' ')}
      style={style}
      data-focus-region={collapsed ? undefined : 'aside'}
      tabIndex={collapsed ? undefined : -1}
      aria-hidden={collapsed || undefined}
      aria-label={ariaLabel}
    >
      {children}
    </aside>
  );
};
