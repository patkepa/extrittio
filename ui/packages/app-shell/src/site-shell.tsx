import type { ReactNode } from "react";
import type { SiteNavAction, SiteNavItem } from "@extrittio/navigation";
import { ErrorBoundary } from "./error-boundary";
import { SiteNavbar } from "./site-navbar";
import "./site-shell.css";

export interface SiteShellProps {
  children: ReactNode;
  productName: string;
  homeHref?: string;
  navItems?: SiteNavItem[];
  actions?: SiteNavAction[];
  navbarUtilities?: ReactNode;
  currentPath?: string;
  navbar?: ReactNode;
  footer?: ReactNode;
  className?: string;
  mainClassName?: string;
  onNavigate?: (href: string) => void;
}

export const SiteShell = ({
  children,
  productName,
  homeHref,
  navItems,
  actions,
  navbarUtilities,
  currentPath,
  navbar,
  footer,
  className,
  mainClassName,
  onNavigate,
}: SiteShellProps) => {
  const shellClassNames = ["site-shell", className].filter(Boolean).join(" ");
  const mainClassNames = ["site-main", mainClassName].filter(Boolean).join(" ");

  return (
    <div className={shellClassNames}>
      {navbar ?? (
        <SiteNavbar
          productName={productName}
          homeHref={homeHref}
          navItems={navItems}
          actions={actions}
          utilities={navbarUtilities}
          currentPath={currentPath}
          onNavigate={onNavigate}
        />
      )}
      <main className={mainClassNames}>
        <ErrorBoundary>{children}</ErrorBoundary>
      </main>
      {footer}
    </div>
  );
};
