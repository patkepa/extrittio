import { useState } from 'react';
import { useNavigate, useLocation } from 'react-router-dom';
import {
  Menu,
  MenuItem,
  MenuDivider,
  Icon,
  Collapse,
  ControlGroup,
  HTMLSelect,
} from '@blueprintjs/core';
import { navGroups, projects } from '../../data/sidebar-data';
import type { NavItem } from '../../types/navigation';
import './app-sidebar.css';

interface AppSidebarProps {
  isCollapsed?: boolean;
  isMobileOpen?: boolean;
  onMobileClose?: () => void;
}

export const AppSidebar = ({ isCollapsed = false, isMobileOpen = false, onMobileClose }: AppSidebarProps) => {
  const navigate = useNavigate();
  const location = useLocation();
  const [expandedItems, setExpandedItems] = useState<Set<string>>(new Set());
  const [selectedProject, setSelectedProject] = useState(projects[0]!);

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
    return location.pathname === href;
  };

  const handleNavigation = (href: string) => {
    navigate(href);
    onMobileClose?.();
  };

  const renderNavItem = (item: NavItem, depth: number = 0) => {
    const hasChildren = item.children && item.children.length > 0;
    const isExpanded = expandedItems.has(item.label);
    const active = isActive(item.href);

    return (
      <div key={item.label} style={{ paddingLeft: `${depth * 16}px` }}>
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
            hasChildren ? (
              <Icon icon={isExpanded ? 'chevron-down' : 'chevron-right'} size={12} />
            ) : undefined
          }
          aria-expanded={hasChildren ? isExpanded : undefined}
          className={active ? 'sidebar-item-active' : ''}
        />
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
          <Icon icon="dashboard" size={24} />
          {!isCollapsed && <span className="sidebar-title">Extrittio</span>}
        </div>
      </div>

      {/* Project Switcher */}
      {!isCollapsed && (
        <div className="sidebar-project-switcher">
          <ControlGroup fill>
            <HTMLSelect
              value={selectedProject.environment}
              onChange={(e) => {
                const project = projects.find(p => p.environment === e.target.value);
                if (project) setSelectedProject(project);
              }}
              options={projects.map(p => p.environment)}
              iconName="double-caret-vertical"
              fill
            />
          </ControlGroup>
        </div>
      )}

      {/* Navigation */}
      <div className="sidebar-nav">
        {navGroups.map((group, idx) => (
          <div key={group.label} className="nav-group">
            {!isCollapsed && (
              <div className="nav-group-label">{group.label.toUpperCase()}</div>
            )}
            <Menu className="sidebar-menu">
              {group.items.map((item) => renderNavItem(item))}
            </Menu>
            {idx < navGroups.length - 1 && <MenuDivider />}
          </div>
        ))}
      </div>
    </div>
  );
};
