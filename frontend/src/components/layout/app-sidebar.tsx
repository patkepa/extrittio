import { useState, useEffect } from 'react';
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
  Button,
} from '@blueprintjs/core';
import { navGroups, projects, currentUser } from '../../data/sidebar-data';
import { useAuthStore } from '../../stores/auth-store';
import { useUIStore } from '../../stores/ui-store';
import { useDashboardStats } from '../../hooks/use-dashboard';
import type { NavItem } from '../../types/navigation';
import './app-sidebar.css';

interface AppSidebarProps {
  isCollapsed?: boolean;
}

const envColors: Record<string, string> = {
  'Development': 'hsl(var(--accent))',
  'Testing': 'hsl(var(--warning))',
  'Production': 'hsl(var(--success))',
};

/** Check if any child route is currently active */
const hasActiveChild = (item: NavItem, pathname: string): boolean => {
  if (!item.children) return false;
  return item.children.some((child) =>
    child.href === '/' ? pathname === '/' : pathname.startsWith(child.href)
  );
};

export const AppSidebar = ({ isCollapsed = false }: AppSidebarProps) => {
  const navigate = useNavigate();
  const location = useLocation();
  const { data: dashboardStats } = useDashboardStats();
  const [expandedItems, setExpandedItems] = useState<Set<string>>(new Set());

  // Auto-expand parent items when a child route is active
  useEffect(() => {
    for (const group of navGroups) {
      for (const item of group.items) {
        if (item.children && hasActiveChild(item, location.pathname)) {
          setExpandedItems((prev) => {
            if (prev.has(item.label)) return prev;
            const next = new Set(prev);
            next.add(item.label);
            return next;
          });
        }
      }
    }
  }, [location.pathname]);
  const [selectedProject, setSelectedProject] = useState(projects[0]!);
  const [projectSwitcherOpen, setProjectSwitcherOpen] = useState(false);
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
    ...(dashboardStats && { 'Devices': { count: dashboardStats.total_devices } }),
    'Alerts': { count: 3 },
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
              {badge && (
                badge.status ? (
                  <span className={`status-led status-led--${badge.status}`} />
                ) : badge.count ? (
                  <Tag minimal round className="nav-count-badge">{badge.count}</Tag>
                ) : null
              )}
              {hasChildren && (
                <Icon icon={isExpanded ? 'chevron-down' : 'chevron-right'} size={12} />
              )}
            </span>
          ) : undefined
        }
        aria-expanded={hasChildren ? isExpanded : undefined}
        className={[
          active && 'sidebar-item-active',
          hasChildren && isExpanded && 'sidebar-item-expanded',
        ].filter(Boolean).join(' ') || undefined}
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
            {!isCollapsed && (
              <div className="nav-group-label">{group.label.toUpperCase()}</div>
            )}
            {isCollapsed && idx > 0 && <MenuDivider />}
            <Menu className="sidebar-menu">
              {group.items.map((item) => renderNavItem(item))}
            </Menu>
          </div>
        ))}
      </div>

      {/* Footer */}
      <div className="sidebar-footer">
        {!isCollapsed ? (
          <>
            <div className="user-card">
              <div className="user-info">
                <div className="user-avatar">{currentUser.name.charAt(0).toUpperCase()}</div>
                <div className="user-details">
                  <div className="user-name">{currentUser.name}</div>
                  <div className="user-email">{currentUser.email}</div>
                </div>
                <Button icon="log-out" minimal small className="user-logout" title="Sign out" onClick={logout} />
              </div>
            </div>
            <div className="sidebar-env-switcher">
              <button
                className="env-switcher-btn"
                onClick={() => setProjectSwitcherOpen(!projectSwitcherOpen)}
              >
                <div className="env-switcher-left">
                  <span
                    className="env-dot"
                    style={{ backgroundColor: envColors[selectedProject.environment] }}
                  />
                  <span className="env-text mono-data">{selectedProject.environment.toUpperCase()}</span>
                </div>
                <div className="env-switcher-right">
                  <span className="version-text mono-data">v0.1.0</span>
                  <Icon icon="double-caret-vertical" size={12} className="env-switcher-caret" />
                </div>
              </button>
              <Collapse isOpen={projectSwitcherOpen}>
                <div className="env-dropdown">
                  {projects.map((project) => (
                    <button
                      key={project.environment}
                      className={`env-option ${selectedProject.environment === project.environment ? 'active' : ''}`}
                      onClick={() => {
                        setSelectedProject(project);
                        setProjectSwitcherOpen(false);
                      }}
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
              </Collapse>
            </div>
          </>
        ) : (
          <Tooltip content={currentUser.name} position={Position.RIGHT} minimal>
            <div className="user-avatar collapsed-avatar">{currentUser.name.charAt(0).toUpperCase()}</div>
          </Tooltip>
        )}
      </div>
    </div>
  );
};
