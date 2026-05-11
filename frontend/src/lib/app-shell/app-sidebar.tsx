import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
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
import {
  clearKeyboardFocusRegions,
  getDirectionalKey,
  shouldIgnorePageShortcut,
} from '../interactions';
import type { NavBadge, NavGroup, NavItem, Project, User } from '../navigation';
import { StatusLed } from '../ui';
import './app-sidebar.css';

export interface AppSidebarProps {
  isCollapsed?: boolean;
  productName: string;
  collapsedProductName?: string;
  navGroups: NavGroup[];
  navBadges?: Record<string, NavBadge>;
  projects?: Project[];
  user?: User;
  version?: string;
  onLogout?: () => void;
  onExpandSidebar?: () => void;
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

function getProjectColor(project: Project) {
  return project.color ?? envColors[project.environment] ?? 'hsl(var(--accent))';
}

export const AppSidebar = ({
  isCollapsed = false,
  productName,
  collapsedProductName,
  navGroups,
  navBadges = {},
  projects = [],
  user,
  version,
  onLogout,
  onExpandSidebar,
}: AppSidebarProps) => {
  const sidebarRef = useRef<HTMLDivElement | null>(null);
  const focusedNavLabelRef = useRef<string | null>(null);
  const prevCollapsedRef = useRef(isCollapsed);
  const navigate = useNavigate();
  const location = useLocation();
  const [expandedItems, setExpandedItems] = useState<Set<string>>(new Set());
  const [selectedProjectEnvironment, setSelectedProjectEnvironment] = useState<string | null>(
    () => projects[0]?.environment ?? null,
  );
  const [footerOpen, setFooterOpen] = useState(false);

  const activeParentLabels = useMemo(() => {
    const labels = new Set<string>();
    for (const group of navGroups) {
      for (const item of group.items) {
        if (item.children && hasActiveChild(item, location.pathname)) {
          labels.add(item.label);
        }
      }
    }
    return labels;
  }, [location.pathname, navGroups]);

  const selectedProject =
    projects.find((project) => project.environment === selectedProjectEnvironment) ??
    projects[0] ??
    null;

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
        onExpandSidebar?.();
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
          onExpandSidebar?.();
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
        onExpandSidebar?.();
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

  const renderNavItem = (item: NavItem, depth: number = 0) => {
    const hasChildren = item.children && item.children.length > 0;
    const isExpanded = expandedItems.has(item.label) || activeParentLabels.has(item.label);
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
                  <StatusLed status={badge.status} />
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
          <span className="sidebar-title">
            {isCollapsed ? (collapsedProductName ?? productName.slice(0, 2)) : productName}
          </span>
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
      {user && (
        <div className="sidebar-footer">
          {!isCollapsed ? (
            <div className={`footer-panel ${footerOpen ? 'open' : ''}`}>
              <button className="footer-panel-trigger" onClick={() => setFooterOpen(!footerOpen)}>
                <div className="footer-trigger-left">
                  <div className="user-avatar">{user.name.charAt(0).toUpperCase()}</div>
                  <div className="user-details">
                    <div className="user-name">{user.name}</div>
                    <div className={`user-email footer-email ${footerOpen ? 'visible' : ''}`}>
                      {user.email}
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
                        className={`env-option ${selectedProject?.environment === project.environment ? 'active' : ''}`}
                        onClick={() => setSelectedProjectEnvironment(project.environment)}
                      >
                        <span
                          className="env-dot"
                          style={{ backgroundColor: getProjectColor(project) }}
                        />
                        <span className="env-option-label">{project.environment}</span>
                        {selectedProject?.environment === project.environment && (
                          <Icon icon="tick" size={12} className="env-option-check" />
                        )}
                      </button>
                    ))}
                  </div>
                  <div className="footer-divider" />
                  {onLogout && (
                    <button className="footer-action" onClick={onLogout}>
                      <Icon icon="log-out" size={14} />
                      <span>Sign out</span>
                    </button>
                  )}
                </div>
              </Collapse>
              {selectedProject && (
                <div className={`footer-env-badge ${footerOpen ? 'hidden' : ''}`}>
                  <span
                    className="env-dot"
                    style={{ backgroundColor: getProjectColor(selectedProject) }}
                  />
                  <span className="env-text mono-data">
                    {selectedProject.environment.toUpperCase()}
                  </span>
                  {version && <span className="version-text mono-data">{version}</span>}
                </div>
              )}
            </div>
          ) : (
            <Popover
              position={Position.RIGHT_TOP}
              minimal
              modifiers={{ offset: { enabled: true, options: { offset: [0, 16] } } }}
              content={
                <div className="collapsed-popover">
                  <div className="collapsed-popover-header">
                    <div className="user-avatar">{user.name.charAt(0).toUpperCase()}</div>
                    <div className="user-details">
                      <div className="user-name">{user.name}</div>
                      <div className="user-email">{user.email}</div>
                    </div>
                  </div>
                  <div className="footer-divider" />
                  <div className="footer-section-label">ENVIRONMENT</div>
                  <div className="footer-env-options">
                    {projects.map((project) => (
                      <button
                        key={project.environment}
                        className={`env-option ${selectedProject?.environment === project.environment ? 'active' : ''}`}
                        onClick={() => setSelectedProjectEnvironment(project.environment)}
                      >
                        <span
                          className="env-dot"
                          style={{ backgroundColor: getProjectColor(project) }}
                        />
                        <span className="env-option-label">{project.environment}</span>
                        {selectedProject?.environment === project.environment && (
                          <Icon icon="tick" size={12} className="env-option-check" />
                        )}
                      </button>
                    ))}
                  </div>
                  <div className="footer-divider" />
                  {onLogout && (
                    <button className="footer-action" onClick={onLogout}>
                      <Icon icon="log-out" size={14} />
                      <span>Sign out</span>
                    </button>
                  )}
                </div>
              }
            >
              <div className="user-avatar collapsed-avatar">
                {user.name.charAt(0).toUpperCase()}
              </div>
            </Popover>
          )}
        </div>
      )}
    </div>
  );
};
