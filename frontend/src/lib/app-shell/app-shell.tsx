import { useEffect, type ReactNode } from 'react';
import { Button, Navbar, NavbarGroup } from '@blueprintjs/core';
import { clearKeyboardFocusRegions, moveFocusRegion } from '../../utils/focus-regions';
import { hasOpenBlockingOverlay, isEditableTarget } from '../../utils/keyboard';
import type { NavBadge, NavGroup, Project, User } from '../navigation';
import { AppSidebar } from './app-sidebar';
import { ErrorBoundary } from './error-boundary';
import './main-layout.css';

export interface AppShellProps {
  children: ReactNode;
  productName: string;
  collapsedProductName?: string;
  navGroups: NavGroup[];
  sidebarCollapsed: boolean;
  onToggleSidebar: () => void;
  breadcrumb?: ReactNode;
  navBadges?: Record<string, NavBadge>;
  projects?: Project[];
  user?: User;
  version?: string;
  onLogout?: () => void;
  onOpenCommandPalette?: () => void;
  commandPalette?: ReactNode;
}

function getFocusRegionDirection(key: string): 'left' | 'right' | null {
  if (key === 'ArrowLeft' || key.toLowerCase() === 'a') return 'left';
  if (key === 'ArrowRight' || key.toLowerCase() === 'd') return 'right';
  return null;
}

export const AppShell = ({
  children,
  productName,
  collapsedProductName,
  navGroups,
  sidebarCollapsed,
  onToggleSidebar,
  breadcrumb,
  navBadges,
  projects,
  user,
  version,
  onLogout,
  onOpenCommandPalette,
  commandPalette,
}: AppShellProps) => {
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'b') {
        e.preventDefault();
        if (e.shiftKey) {
          window.dispatchEvent(new CustomEvent('toggle-right-sidebar'));
        } else {
          onToggleSidebar();
        }
        return;
      }

      const focusRegionDirection = getFocusRegionDirection(e.key);
      if (
        focusRegionDirection &&
        e.shiftKey &&
        !e.metaKey &&
        !e.ctrlKey &&
        !e.altKey &&
        !e.defaultPrevented &&
        !e.isComposing &&
        !isEditableTarget(e.target) &&
        !hasOpenBlockingOverlay()
      ) {
        e.preventDefault();
        moveFocusRegion(focusRegionDirection);
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [onToggleSidebar]);

  return (
    <div className="main-layout">
      <AppSidebar
        isCollapsed={sidebarCollapsed}
        productName={productName}
        collapsedProductName={collapsedProductName}
        navGroups={navGroups}
        navBadges={navBadges}
        projects={projects}
        user={user}
        version={version}
        onLogout={onLogout}
        onExpandSidebar={onToggleSidebar}
      />

      <div className="main-content">
        <Navbar className="top-navbar">
          <NavbarGroup>
            <Button
              className="desktop-collapse-button"
              icon={sidebarCollapsed ? 'double-chevron-right' : 'double-chevron-left'}
              minimal
              onClick={onToggleSidebar}
              title="Toggle Sidebar (⌘B)"
            />
            {breadcrumb && <span className="navbar-breadcrumb">{breadcrumb}</span>}
          </NavbarGroup>

          <NavbarGroup align="right">
            {onOpenCommandPalette && (
              <Button icon="search" minimal title="Search (⌘K)" onClick={onOpenCommandPalette} />
            )}
          </NavbarGroup>
        </Navbar>

        <div
          className="page-content"
          data-focus-region="main"
          tabIndex={-1}
          onMouseDown={() => clearKeyboardFocusRegions()}
        >
          <ErrorBoundary>{children}</ErrorBoundary>
        </div>
      </div>

      {commandPalette}
    </div>
  );
};
