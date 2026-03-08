import React, { useState } from 'react';
import { useLocation } from 'react-router-dom';
import { Button, Navbar, NavbarGroup, NavbarDivider, Tag } from '@blueprintjs/core';
import { AppSidebar } from './app-sidebar';
import './main-layout.css';

interface MainLayoutProps {
  children: React.ReactNode;
}

const routeNames: Record<string, string> = {
  '/': 'Dashboard',
  '/devices': 'Devices',
  '/settings': 'Settings',
  '/users': 'Users',
  '/help-center': 'Help Center',
};

export const MainLayout = ({ children }: MainLayoutProps) => {
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [mobileSidebarOpen, setMobileSidebarOpen] = useState(false);
  const location = useLocation();

  const currentRoute = routeNames[location.pathname] ?? location.pathname.split('/').filter(Boolean).map(s => s.charAt(0).toUpperCase() + s.slice(1)).join(' / ');

  return (
    <div className="main-layout">
      <AppSidebar
        isCollapsed={sidebarCollapsed}
        isMobileOpen={mobileSidebarOpen}
        onMobileClose={() => setMobileSidebarOpen(false)}
      />
      {mobileSidebarOpen && (
        <div
          className="mobile-sidebar-overlay"
          onClick={() => setMobileSidebarOpen(false)}
        />
      )}

      <div className="main-content">
        <Navbar className="top-navbar">
          <NavbarGroup>
            <Button
              className="mobile-menu-button"
              icon="menu"
              minimal
              onClick={() => setMobileSidebarOpen(!mobileSidebarOpen)}
              title="Open Menu"
            />
            <Button
              className="desktop-collapse-button"
              icon={sidebarCollapsed ? "double-chevron-right" : "double-chevron-left"}
              minimal
              onClick={() => setSidebarCollapsed(!sidebarCollapsed)}
              title="Toggle Sidebar"
            />
            <NavbarDivider />
            <span className="navbar-breadcrumb">{currentRoute}</span>
          </NavbarGroup>

          <NavbarGroup align="right">
            <Tag minimal className="navbar-status">
              <span className="status-led status-led--online" />
              <span>All Systems Operational</span>
            </Tag>
            <NavbarDivider />
            <div className="notification-wrapper">
              <Button icon="notifications" minimal title="Notifications" />
              <span className="notification-badge">3</span>
            </div>
            <Button icon="search" minimal title="Search" />
          </NavbarGroup>
        </Navbar>

        <div className="page-content">
          {children}
        </div>
      </div>
    </div>
  );
};
