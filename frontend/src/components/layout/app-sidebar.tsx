import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { useNavigate, useLocation } from 'react-router-dom';
import {
  Menu,
  MenuItem,
  MenuDivider,
  Icon,
  Collapse,
  Tag,
  Tooltip,
  Position,
  Popover,
} from '@blueprintjs/core';
import { projects, currentUser } from '../../data/sidebar-data';
import { navGroups } from '../../app/routes';
import { useAuthStore } from '../../stores/auth-store';
import { useUIStore } from '../../stores/ui-store';
import { useDashboardStats } from '../../hooks/use-dashboard';
import { useAlertSummary } from '../../hooks/use-alerts';
import { clearKeyboardFocusRegions } from '../../utils/focus-regions';
import { getDirectionalKey, shouldIgnorePageShortcut } from '../../utils/keyboard';
import type { NavItem } from '../../types/navigation';
import './app-sidebar.css';

interface AppSidebarProps {
  isCollapsed?: boolean;
}

const envColors: Record<string, string> = {
  Development: 'hsl(var(--accent))',
  Testing: 'hsl(var(--warning))',
  Production: 'hsl(var(--success))',
};

/** Check if any child route is currently active */
const hasActiveChild = (item: NavItem, pathname: string): boolean => {
  if (!item.children) return false;
  return item.children.some((child) =>
    child.href === '/' ? pathname === '/' : pathname.startsWith(child.href),
  );
};

const SIDEBAR_NAV_ITEM_SELECTOR = '[data-sidebar-nav-item="true"]';
const isActivationKey = (key: string) => key === 'Enter' || key === ' ' || key === 'Spacebar';

export const AppSidebar = ({ isCollapsed = false }: AppSidebarProps) => {
  const sidebarRef = useRef<HTMLDivElement | null>(null);
  const focusedNavLabelRef = useRef<string | null>(null);
  const prevCollapsedRef = useRef(isCollapsed);
  const navigate = useNavigate();
  const location = useLocation();
  const { data: dashboardStats } = useDashboardStats();
  const { data: alertSummary } = useAlertSummary();
  const [expandedItems, setExpandedItems] = useState<Set<string>>(new Set());

  // Auto-expand parent items when the active route changes
  const [prevPathname, setPrevPathname] = useState(location.pathname);
  if (prevPathname !== location.pathname) {
    setPrevPathname(location.pathname);
    let updated = expandedItems;
    for (const group of navGroups) {
      for (const item of group.items) {
        if (item.children && hasActiveChild(item, location.pathname) && !updated.has(item.label)) {
          if (updated === expandedItems) updated = new Set(expandedItems);
          updated.add(item.label);
        }
      }
    }
    if (updated !== expandedItems) {
      setExpandedItems(updated);
    }
  }
  const [selectedProject, setSelectedProject] = useState(projects[0]!);
  const [footerOpen, setFooterOpen] = useState(false);
  const logout = useAuthStore((s) => s.logout);
  const expandSidebar = useUIStore((s) => s.toggleSidebar);

  const toggleExpanded = (label: string) => {
    setExpandedItems((prev) => {
      const next = new Set(prev);
      if (next.has(label)) {
        next.delete(label);
      } else {
        next.add(label);
      }
      return next;
    });
  };

  const isActive = (href: string) => {
    if (href === '/') return location.pathname === '/';
    return location.pathname.startsWith(href);
  };

  const handleNavigation = (href: string) => {
    navigate(href);
  };

  const activateNavItem = (item: NavItem) => {
    const hasChildren = item.children && item.children.length > 0;

    if (hasChildren) {
      if (isCollapsed) {
        expandSidebar();
      }
      toggleExpanded(item.label);
    } else {
      handleNavigation(item.href);
    }
  };

  const getNavElements = useCallback(
    () =>
      Array.from(
        sidebarRef.current?.querySelectorAll<HTMLElement>(SIDEBAR_NAV_ITEM_SELECTOR) ?? [],
      ),
    [],
  );

  const focusNavElement = useCallback(
    (index: number) => {
      const navElements = getNavElements();
      if (navElements.length === 0) return;

      const nextIndex = Math.min(Math.max(index, 0), navElements.length - 1);
      navElements[nextIndex]?.focus();
    },
    [getNavElements],
  );

  const focusActiveOrFirstNavElement = useCallback(() => {
    const navElements = getNavElements();
    const activeIndex = navElements.findIndex((element) =>
      element.classList.contains('sidebar-item-active'),
    );
    focusNavElement(activeIndex >= 0 ? activeIndex : 0);
  }, [focusNavElement, getNavElements]);

  useEffect(() => {
    const handleDocumentKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'F6' || shouldIgnorePageShortcut(event)) return;

      event.preventDefault();
      focusActiveOrFirstNavElement();
    };

    document.addEventListener('keydown', handleDocumentKeyDown);
    return () => document.removeEventListener('keydown', handleDocumentKeyDown);
  }, [focusActiveOrFirstNavElement]);

  // Restore focus when sidebar collapse state changes.
  // DOM elements are recreated (Tooltip wrap/unwrap), which moves focus to body.
  useLayoutEffect(() => {
    if (prevCollapsedRef.current === isCollapsed) return;
    prevCollapsedRef.current = isCollapsed;

    // Only restore if focus was lost (removed element → focus moved to body)
    const active = document.activeElement;
    if (active !== document.body && active !== document.documentElement) return;

    const label = focusedNavLabelRef.current;
    if (!label) return;

    if (label === '__sidebar__') {
      sidebarRef.current?.focus();
      return;
    }

    const navElements = getNavElements();
    const match = navElements.find((el) => el.dataset.label === label);
    if (match) {
      match.focus();
    } else {
      focusActiveOrFirstNavElement();
    }
  }, [focusActiveOrFirstNavElement, getNavElements, isCollapsed]);

  const handleSidebarMouseDown = (event: React.MouseEvent<HTMLDivElement>) => {
    clearKeyboardFocusRegions();

    if (!(event.target instanceof HTMLElement)) return;

    const navItem = event.target.closest<HTMLElement>(SIDEBAR_NAV_ITEM_SELECTOR);
    if (navItem) {
      navItem.focus();
      return;
    }

    if (event.target.closest('button, a, input, select, textarea, [role="button"]')) return;

    sidebarRef.current?.focus();
  };

  const handleSidebarKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (event.defaultPrevented) return;

    const currentItem =
      event.target instanceof HTMLElement
        ? event.target.closest<HTMLElement>(SIDEBAR_NAV_ITEM_SELECTOR)
        : null;
    if (!currentItem) {
      const direction = getDirectionalKey(event);
      if (!direction && !isActivationKey(event.key)) return;

      event.preventDefault();
      focusActiveOrFirstNavElement();
      return;
    }

    if (isActivationKey(event.key)) {
      event.preventDefault();
      const hasChildren = currentItem.dataset.hasChildren === 'true';
      const label = currentItem.dataset.label;
      const href = currentItem.dataset.href;

      if (hasChildren && label) {
        if (isCollapsed) {
          expandSidebar();
        }
        toggleExpanded(label);
      } else if (href) {
        handleNavigation(href);
      }
      return;
    }

    const direction = getDirectionalKey(event);
    if (!direction) return;

    const navElements = getNavElements();
    const currentIndex = navElements.indexOf(currentItem);
    if (currentIndex === -1) return;

    const hasChildren = currentItem.dataset.hasChildren === 'true';
    const isExpanded = currentItem.dataset.expanded === 'true';
    const label = currentItem.dataset.label;

    if (direction === 'right' && hasChildren && !isExpanded) {
      event.preventDefault();
      if (isCollapsed) {
        expandSidebar();
      }
      if (label) {
        setExpandedItems((prev) => new Set(prev).add(label));
      }
      return;
    }

    if (direction === 'left' && hasChildren && isExpanded && label) {
      event.preventDefault();
      setExpandedItems((prev) => {
        const next = new Set(prev);
        next.delete(label);
        return next;
      });
      return;
    }

    let nextIndex = currentIndex;
    if (direction === 'down' || direction === 'right') nextIndex = currentIndex + 1;
    if (direction === 'up' || direction === 'left') nextIndex = currentIndex - 1;
    if (direction === 'first') nextIndex = 0;
    if (direction === 'last') nextIndex = navElements.length - 1;

    event.preventDefault();
    focusNavElement(nextIndex);
  };

  const navBadges: Record<string, { count?: number; status?: 'online' | 'warning' | 'offline' }> = {
    ...(dashboardStats && { Devices: { count: dashboardStats.total_devices } }),
    ...(alertSummary &&
      alertSummary.total_active > 0 && { Alerts: { count: alertSummary.total_active } }),
  };

  const renderNavItem = (item: NavItem, depth: number = 0) => {
    const hasChildren = item.children && item.children.length > 0;
    const isExpanded = expandedItems.has(item.label);
    // Don't highlight parent items — only leaf items should show active state
    const active = hasChildren ? false : isActive(item.href);
    const badge = navBadges[item.label];

    const menuItem = (
      <MenuItem
        icon={item.icon}
        text={!isCollapsed ? item.label : undefined}
        active={active}
        onClick={() => activateNavItem(item)}
        onKeyDown={(event) => {
          if (!isActivationKey(event.key)) return;

          event.preventDefault();
          event.stopPropagation();
          activateNavItem(item);
        }}
        labelElement={
          !isCollapsed ? (
            <span className="nav-item-right">
              {badge &&
                (badge.status ? (
                  <span className={`status-led status-led--${badge.status}`} />
                ) : badge.count ? (
                  <Tag minimal className="nav-count-badge">
                    {badge.count}
                  </Tag>
                ) : null)}
              {hasChildren && (
                <Icon icon={isExpanded ? 'chevron-down' : 'chevron-right'} size={12} />
              )}
            </span>
          ) : undefined
        }
        aria-expanded={hasChildren ? isExpanded : undefined}
        data-sidebar-nav-item="true"
        data-has-children={hasChildren ? 'true' : undefined}
        data-expanded={hasChildren ? String(isExpanded) : undefined}
        data-focus-region-initial={active ? 'true' : undefined}
        data-label={item.label}
        data-href={item.href}
        className={
          [active && 'sidebar-item-active', hasChildren && isExpanded && 'sidebar-item-expanded']
            .filter(Boolean)
            .join(' ') || undefined
        }
      />
    );

    return (
      <div key={item.label} style={{ paddingLeft: `${depth * 16}px` }}>
        {isCollapsed ? (
          <Tooltip content={item.label} position={Position.RIGHT} minimal>
            {menuItem}
          </Tooltip>
        ) : (
          menuItem
        )}
        {hasChildren && !isCollapsed && (
          <Collapse isOpen={isExpanded}>
            <div className="sidebar-submenu">
              {item.children!.map((child) => renderNavItem(child, depth + 1))}
            </div>
          </Collapse>
        )}
      </div>
    );
  };

  return (
    <div
      className={`app-sidebar ${isCollapsed ? 'collapsed' : ''}`}
      ref={sidebarRef}
      data-focus-region="sidebar"
      tabIndex={-1}
      onKeyDown={handleSidebarKeyDown}
      onMouseDown={handleSidebarMouseDown}
      onFocusCapture={(e) => {
        const navItem = (e.target as HTMLElement).closest<HTMLElement>(SIDEBAR_NAV_ITEM_SELECTOR);
        focusedNavLabelRef.current = navItem ? (navItem.dataset.label ?? null) : '__sidebar__';
      }}
    >
      {/* Header */}
      <div className="sidebar-header">
        <div className="sidebar-logo">
          <span className="sidebar-title">{isCollapsed ? 'Ex' : 'Extrittio'}</span>
        </div>
      </div>

      {/* Navigation */}
      <div className="sidebar-nav">
        {navGroups.map((group, idx) => (
          <div key={group.label} className="nav-group">
            {!isCollapsed && <div className="nav-group-label">{group.label.toUpperCase()}</div>}
            {isCollapsed && idx > 0 && <MenuDivider />}
            <Menu className="sidebar-menu">{group.items.map((item) => renderNavItem(item))}</Menu>
          </div>
        ))}
      </div>

      {/* Footer */}
      <div className="sidebar-footer">
        {!isCollapsed ? (
          <div className={`footer-panel ${footerOpen ? 'open' : ''}`}>
            <button className="footer-panel-trigger" onClick={() => setFooterOpen(!footerOpen)}>
              <div className="footer-trigger-left">
                <div className="user-avatar">{currentUser.name.charAt(0).toUpperCase()}</div>
                <div className="user-details">
                  <div className="user-name">{currentUser.name}</div>
                  <div className={`user-email footer-email ${footerOpen ? 'visible' : ''}`}>
                    {currentUser.email}
                  </div>
                </div>
              </div>
              <Icon icon="double-caret-vertical" size={12} className="footer-panel-caret" />
            </button>
            <Collapse isOpen={footerOpen}>
              <div className="footer-panel-content">
                <div className="footer-section-label">ENVIRONMENT</div>
                <div className="footer-env-options">
                  {projects.map((project) => (
                    <button
                      key={project.environment}
                      className={`env-option ${selectedProject.environment === project.environment ? 'active' : ''}`}
                      onClick={() => setSelectedProject(project)}
                    >
                      <span
                        className="env-dot"
                        style={{ backgroundColor: envColors[project.environment] }}
                      />
                      <span className="env-option-label">{project.environment}</span>
                      {selectedProject.environment === project.environment && (
                        <Icon icon="tick" size={12} className="env-option-check" />
                      )}
                    </button>
                  ))}
                </div>
                <div className="footer-divider" />
                <button className="footer-action" onClick={logout}>
                  <Icon icon="log-out" size={14} />
                  <span>Sign out</span>
                </button>
              </div>
            </Collapse>
            <div className={`footer-env-badge ${footerOpen ? 'hidden' : ''}`}>
              <span
                className="env-dot"
                style={{ backgroundColor: envColors[selectedProject.environment] }}
              />
              <span className="env-text mono-data">
                {selectedProject.environment.toUpperCase()}
              </span>
              <span className="version-text mono-data">v0.1.0</span>
            </div>
          </div>
        ) : (
          <Popover
            position={Position.RIGHT_TOP}
            minimal
            modifiers={{ offset: { enabled: true, options: { offset: [0, 16] } } }}
            content={
              <div className="collapsed-popover">
                <div className="collapsed-popover-header">
                  <div className="user-avatar">{currentUser.name.charAt(0).toUpperCase()}</div>
                  <div className="user-details">
                    <div className="user-name">{currentUser.name}</div>
                    <div className="user-email">{currentUser.email}</div>
                  </div>
                </div>
                <div className="footer-divider" />
                <div className="footer-section-label">ENVIRONMENT</div>
                <div className="footer-env-options">
                  {projects.map((project) => (
                    <button
                      key={project.environment}
                      className={`env-option ${selectedProject.environment === project.environment ? 'active' : ''}`}
                      onClick={() => setSelectedProject(project)}
                    >
                      <span
                        className="env-dot"
                        style={{ backgroundColor: envColors[project.environment] }}
                      />
                      <span className="env-option-label">{project.environment}</span>
                      {selectedProject.environment === project.environment && (
                        <Icon icon="tick" size={12} className="env-option-check" />
                      )}
                    </button>
                  ))}
                </div>
                <div className="footer-divider" />
                <button className="footer-action" onClick={logout}>
                  <Icon icon="log-out" size={14} />
                  <span>Sign out</span>
                </button>
              </div>
            }
          >
            <div className="user-avatar collapsed-avatar">
              {currentUser.name.charAt(0).toUpperCase()}
            </div>
          </Popover>
        )}
      </div>
    </div>
  );
};
