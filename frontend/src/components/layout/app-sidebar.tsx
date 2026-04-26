import { useState } from 'react';
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
import { navGroups, projects, currentUser } from '../../data/sidebar-data';
import { useAuthStore } from '../../stores/auth-store';
import { useUIStore } from '../../stores/ui-store';
import { useDashboardStats } from '../../hooks/use-dashboard';
import { useAlertSummary } from '../../hooks/use-alerts';
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

export const AppSidebar = ({ isCollapsed = false }: AppSidebarProps) => {
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

  const navBadges: Record<string, { count?: number; status?: 'online' | 'warning' | 'offline' }> = {
    ...(dashboardStats && { Devices: { count: dashboardStats.total_devices } }),
    ...(alertSummary && alertSummary.total_active > 0 && { Alerts: { count: alertSummary.total_active } }),
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
        onClick={() => {
          if (hasChildren) {
            if (isCollapsed) {
              expandSidebar();
            }
            toggleExpanded(item.label);
          } else {
            handleNavigation(item.href);
          }
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
    <div className={`app-sidebar ${isCollapsed ? 'collapsed' : ''}`}>
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
            <button
              className="footer-panel-trigger"
              onClick={() => setFooterOpen(!footerOpen)}
            >
              <div className="footer-trigger-left">
                <div className="user-avatar">
                  {currentUser.name.charAt(0).toUpperCase()}
                </div>
                <div className="user-details">
                  <div className="user-name">{currentUser.name}</div>
                  <div className={`user-email footer-email ${footerOpen ? 'visible' : ''}`}>{currentUser.email}</div>
                </div>
              </div>
              <Icon
                icon="double-caret-vertical"
                size={12}
                className="footer-panel-caret"
              />
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
                  <div className="user-avatar">
                    {currentUser.name.charAt(0).toUpperCase()}
                  </div>
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
