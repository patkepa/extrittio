import type { CSSProperties, ReactNode } from "react";

export interface SiteGridProps {
  children: ReactNode;
  minColumnWidth?: string;
  className?: string;
  ariaLabel?: string;
}

export const SiteGrid = ({
  children,
  minColumnWidth = "240px",
  className,
  ariaLabel,
}: SiteGridProps) => {
  const classNames = ["site-grid", className].filter(Boolean).join(" ");
  const style = {
    "--site-grid-min": minColumnWidth,
  } as CSSProperties;

  return (
    <div className={classNames} style={style} aria-label={ariaLabel}>
      {children}
    </div>
  );
};
