import { useState } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';
import { useDevice } from '../../hooks/use-devices';
import { Button, Navbar, NavbarGroup } from '@blueprintjs/core';
import { AppSidebar } from './app-sidebar';
import { CommandPalette } from '../command-palette/command-palette';
import { useUIStore } from '../../stores/ui-store';
import { ErrorBoundary } from '../error-boundary';
import './main-layout.css';

interface MainLayoutProps {
  children: React.ReactNode;
}

const routeNames: Record<string, string> = {
  '/': 'Dashboard',
  '/devices': 'Devices',
  '/settings': 'Settings',
  '/settings/profile': 'Settings / Profile',
  '/settings/device-types': 'Settings / Device Types',
  '/settings/fleets': 'Settings / Fleets',
};

export const MainLayout = ({ children }: MainLayoutProps) => {
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const location = useLocation();
  const navigate = useNavigate();
  const openCommandPalette = useUIStore((s) => s.openCommandPalette);

  // Detect device detail page: /devices/:deviceId
  const deviceDetailMatch = location.pathname.match(/^\/devices\/([^/]+)$/);
  const deviceId = deviceDetailMatch?.[1] ?? null;
  const { data: deviceData } = useDevice(deviceId);

  const currentRoute = deviceDetailMatch
    ? null
    : routeNames[location.pathname]
      ?? location.pathname.split('/').filter(Boolean).map(s => s.charAt(0).toUpperCase() + s.slice(1)).join(' / ');

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
            {deviceDetailMatch ? (
              <span className="navbar-breadcrumb">
                <span
                  className="breadcrumb-link"
                  onClick={() => navigate('/devices')}
                >
                  Devices
                </span>
                <span className="breadcrumb-sep"> / </span>
                <span>{deviceData?.name ?? deviceId}</span>
              </span>
            ) : (
              <span className="navbar-breadcrumb">{currentRoute}</span>
            )}
          </NavbarGroup>

          <NavbarGroup align="right">
            <Button
              icon="search"
              minimal
              title="Search (⌘K)"
              onClick={openCommandPalette}
            />
          </NavbarGroup>
        </Navbar>

        <div className="page-content">
          <ErrorBoundary>
            {children}
          </ErrorBoundary>
        </div>
      </div>

      <CommandPalette />
    </div>
  );
};
