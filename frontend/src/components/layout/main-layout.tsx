import React, { useState } from 'react';
import { Button, Navbar, NavbarGroup } from '@blueprintjs/core';
import { AppSidebar } from './app-sidebar';
import './main-layout.css';

interface MainLayoutProps {
  children: React.ReactNode;
}

export const MainLayout = ({ children }: MainLayoutProps) => {
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);

  return (
    <div className="main-layout">
      <AppSidebar isCollapsed={sidebarCollapsed} />

      <div className="main-content">
        {/* Top Navigation Bar */}
        <Navbar className="top-navbar">
          <NavbarGroup>
            <Button
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

        {/* Page Content */}
        <div className="page-content">
          {children}
        </div>
      </div>
    </div>
  );
};
