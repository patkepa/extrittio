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
  Button,
} from '@blueprintjs/core';
import { navGroups, projects, currentUser } from '../../data/sidebar-data';
import { useDashboardStats } from '../../hooks/use-dashboard';
import type { NavItem } from '../../types/navigation';
import './app-sidebar.css';

interface AppSidebarProps {
  isCollapsed?: boolean;
  isMobileOpen?: boolean;
  onMobileClose?: () => void;
}

const envColors: Record<string, string> = {
  'Development': 'hsl(var(--accent))',
  'Testing': 'hsl(var(--warning))',
  'Production': 'hsl(var(--success))',
};

export const AppSidebar = ({ isCollapsed = false, isMobileOpen = false, onMobileClose }: AppSidebarProps) => {
  const navigate = useNavigate();
  const location = useLocation();
  const { data: dashboardStats } = useDashboardStats();
  const [expandedItems, setExpandedItems] = useState<Set<string>>(new Set());
  const [selectedProject, setSelectedProject] = useState(projects[0]!);
  const [projectSwitcherOpen, setProjectSwitcherOpen] = useState(false);

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
    onMobileClose?.();
  };

  const navBadges: Record<string, { count?: number; status?: 'online' | 'warning' | 'offline' }> = {
    'Dashboard': { status: 'online' },
    ...(dashboardStats && { 'Devices': { count: dashboardStats.total_devices } }),
    'Alerts': { count: 3 },
  };

  const renderNavItem = (item: NavItem, depth: number = 0) => {
    const hasChildren = item.children && item.children.length > 0;
    const isExpanded = expandedItems.has(item.label);
    const active = isActive(item.href);
    const badge = navBadges[item.label];

    const menuItem = (
      <MenuItem
        icon={item.icon}
        text={!isCollapsed ? item.label : undefined}
        active={active}
        onClick={() => {
          if (hasChildren) {
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
        className={active ? 'sidebar-item-active' : ''}
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
    <div className={`app-sidebar ${isCollapsed ? 'collapsed' : ''} ${isMobileOpen ? 'mobile-open' : ''}`}>
      {/* Header */}
      <div className="sidebar-header">
        <div className="sidebar-logo">
          <div className="logo-icon">
            <Icon icon="cube" size={18} />
          </div>
          {!isCollapsed && <span className="sidebar-title">Extrittio</span>}
        </div>
      </div>

      {/* Project Switcher */}
      {!isCollapsed && (
        <div className="sidebar-project-switcher">
          <button
            className="project-switcher-btn"
            onClick={() => setProjectSwitcherOpen(!projectSwitcherOpen)}
          >
            <div className="project-switcher-left">
              <div className="project-icon-wrapper" style={{ borderColor: envColors[selectedProject.environment] }}>
                <Icon icon={selectedProject.icon} size={14} />
              </div>
              <div className="project-switcher-info">
                <span className="project-switcher-name">{selectedProject.name}</span>
                <span className="project-switcher-env mono-data">{selectedProject.environment.toUpperCase()}</span>
              </div>
            </div>
            <Icon icon="double-caret-vertical" size={14} className="project-switcher-caret" />
          </button>
          <Collapse isOpen={projectSwitcherOpen}>
            <div className="project-dropdown">
              {projects.map((project) => (
                <button
                  key={project.environment}
                  className={`project-option ${selectedProject.environment === project.environment ? 'active' : ''}`}
                  onClick={() => {
                    setSelectedProject(project);
                    setProjectSwitcherOpen(false);
                  }}
                >
                  <div className="project-option-icon" style={{ borderColor: envColors[project.environment] }}>
                    <Icon icon={project.icon} size={12} />
                  </div>
                  <div className="project-option-info">
                    <span className="project-option-name">{project.name}</span>
                    <span className="project-option-env mono-data">{project.environment}</span>
                  </div>
                  {selectedProject.environment === project.environment && (
                    <Icon icon="tick" size={14} className="project-option-check" />
                  )}
                </button>
              ))}
            </div>
          </Collapse>
        </div>
      )}

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
            <div className="sidebar-env-badge">
              <span
                className="env-dot"
                style={{ backgroundColor: envColors[selectedProject.environment] }}
              />
              <span className="env-text mono-data">{selectedProject.environment.toUpperCase()}</span>
              <span className="version-text mono-data">v0.1.0</span>
            </div>
            <div className="user-card">
              <div className="user-info">
                <div className="user-avatar">{currentUser.name.charAt(0).toUpperCase()}</div>
                <div className="user-details">
                  <div className="user-name">{currentUser.name}</div>
                  <div className="user-email">{currentUser.email}</div>
                </div>
                <Button icon="log-out" minimal small className="user-logout" title="Sign out" />
              </div>
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
