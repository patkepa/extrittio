import { useState } from 'react';
import { useLocation } from 'react-router-dom';
import { Button, Navbar, NavbarGroup } from '@blueprintjs/core';
import { AppSidebar } from './app-sidebar';
import { CommandPalette } from '../command-palette/command-palette';
import './main-layout.css';

interface MainLayoutProps {
  children: React.ReactNode;
}

const routeNames: Record<string, string> = {
  '/': 'Dashboard',
  '/devices': 'Devices',
  '/settings': 'Settings',
  '/settings/device-types': 'Settings / Device Types',
  '/settings/fleets': 'Settings / Fleets',
  '/users': 'Users',
  '/help-center': 'Help Center',
};

export const MainLayout = ({ children }: MainLayoutProps) => {
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const location = useLocation();

  const currentRoute = routeNames[location.pathname] ?? location.pathname.split('/').filter(Boolean).map(s => s.charAt(0).toUpperCase() + s.slice(1)).join(' / ');

  return (
    <div className="main-layout">
      <AppSidebar
        isCollapsed={sidebarCollapsed}
      />

      <div className="main-content">
        <Navbar className="top-navbar">
          <NavbarGroup>
            <Button
              className="desktop-collapse-button"
              icon={sidebarCollapsed ? "double-chevron-right" : "double-chevron-left"}
              minimal
              onClick={() => setSidebarCollapsed(!sidebarCollapsed)}
              title="Toggle Sidebar"
            />
            <span className="navbar-breadcrumb">{currentRoute}</span>
          </NavbarGroup>

          <NavbarGroup align="right">
            <div className="notification-wrapper">
              <Button icon="notifications" minimal title="Notifications" />
              <span className="notification-badge">3</span>
            </div>
            <Button
              icon="search"
              minimal
              title="Search (⌘K)"
              onClick={() => {
                document.dispatchEvent(new KeyboardEvent('keydown', { key: 'k', metaKey: true }));
              }}
            />
          </NavbarGroup>
        </Navbar>

        <div className="page-content">
          {children}
        </div>
      </div>

      <CommandPalette />
    </div>
  );
};
