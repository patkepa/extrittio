import type { ReactNode } from "react";

export interface BottomToolbarProps {
  children: ReactNode;
  className?: string;
  ariaLabel?: string;
}

export const BottomToolbar = ({
  children,
  className,
  ariaLabel = "Bottom toolbar",
}: BottomToolbarProps) => {
  const classNames = ["bottom-toolbar", className].filter(Boolean).join(" ");

  return (
    <div className={classNames} role="toolbar" aria-label={ariaLabel}>
      {children}
    </div>
  );
};
