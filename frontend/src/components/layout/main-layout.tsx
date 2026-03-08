import React, { useState } from 'react';
import { Button, Navbar, NavbarGroup } from '@blueprintjs/core';
import { AppSidebar } from './app-sidebar';
import './main-layout.css';

interface MainLayoutProps {
  children: React.ReactNode;
}

export const MainLayout = ({ children }: MainLayoutProps) => {
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [mobileSidebarOpen, setMobileSidebarOpen] = useState(false);

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
          </NavbarGroup>

          <NavbarGroup align="right">
            <Button icon="notifications" minimal title="Notifications" />
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
