import { useState } from 'react';
import { useLocation } from 'react-router-dom';
import { PanelLeftClose, PanelLeftOpen, Bell, Search } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { AppSidebar } from './app-sidebar';

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
  const location = useLocation();

  const currentRoute =
    routeNames[location.pathname] ??
    location.pathname
      .split('/')
      .filter(Boolean)
      .map((s) => s.charAt(0).toUpperCase() + s.slice(1))
      .join(' / ');

  return (
    <div className="flex h-screen overflow-hidden bg-background">
      <AppSidebar isCollapsed={sidebarCollapsed} />

      <div className="flex flex-1 min-w-0 flex-col overflow-hidden bg-background">
        <header className="flex items-center justify-between h-12 px-4 border-b border-border bg-background shrink-0">
          <div className="flex items-center gap-2">
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setSidebarCollapsed(!sidebarCollapsed)}
              title="Toggle Sidebar"
              className="h-7 w-7 p-0"
            >
              {sidebarCollapsed ? (
                <PanelLeftOpen size={16} />
              ) : (
                <PanelLeftClose size={16} />
              )}
            </Button>
            <span className="text-[13px] font-semibold text-foreground tracking-tight">
              {currentRoute}
            </span>
          </div>

          <div className="flex items-center gap-1">
            <div className="relative inline-flex">
              <Button variant="ghost" size="sm" title="Notifications" className="h-7 w-7 p-0">
                <Bell size={16} />
              </Button>
              <span className="absolute top-0.5 right-0.5 w-4 h-4 rounded-full bg-danger text-white text-[9px] font-bold flex items-center justify-center pointer-events-none font-mono">
                3
              </span>
            </div>
            <Button variant="ghost" size="sm" title="Search" className="h-7 w-7 p-0">
              <Search size={16} />
            </Button>
          </div>
        </header>

        <div className="flex-1 overflow-y-auto p-6 bg-background max-md:p-4">
          {children}
        </div>
      </div>
    </div>
  );
};
