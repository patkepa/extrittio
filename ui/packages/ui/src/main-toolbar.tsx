import type { ReactNode } from "react";

export interface MainToolbarProps {
  children: ReactNode;
  className?: string;
  ariaLabel?: string;
}

export const MainToolbar = ({
  children,
  className,
  ariaLabel = "Main toolbar",
}: MainToolbarProps) => {
  const classNames = ["main-toolbar", className].filter(Boolean).join(" ");

  return (
    <div className={classNames} role="toolbar" aria-label={ariaLabel}>
      {children}
    </div>
  );
};
