import type { ReactNode } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';
import { CommandPalette } from '../components/command-palette/command-palette';
import { currentUser, projects } from '../data/sidebar-data';
import { useAlertSummary } from '../hooks/use-alerts';
import { useDashboardStats } from '../hooks/use-dashboard';
import { useDevice } from '../hooks/use-devices';
import { AppShell } from '../lib/app-shell';
import { useAuthStore } from '../stores/auth-store';
import { useUIStore } from '../stores/ui-store';
import type { NavBadge } from '../types/navigation';
import { getRouteLabel, navGroups } from './routes';

interface ExtrittioShellProps {
  children: ReactNode;
}

export const ExtrittioShell = ({ children }: ExtrittioShellProps) => {
  const location = useLocation();
  const navigate = useNavigate();
  const logout = useAuthStore((s) => s.logout);
  const openCommandPalette = useUIStore((s) => s.openCommandPalette);
  const sidebarCollapsed = useUIStore((s) => s.isSidebarCollapsed);
  const toggleSidebar = useUIStore((s) => s.toggleSidebar);
  const { data: dashboardStats } = useDashboardStats();
  const { data: alertSummary } = useAlertSummary();

  const deviceDetailMatch = location.pathname.match(/^\/devices\/([^/]+)$/);
  const deviceId = deviceDetailMatch?.[1] ?? null;
  const { data: deviceData } = useDevice(deviceId);
  const currentRoute = deviceDetailMatch ? null : getRouteLabel(location.pathname);

  const navBadges: Record<string, NavBadge> = {
    ...(dashboardStats && { Devices: { count: dashboardStats.total_devices } }),
    ...(alertSummary &&
      alertSummary.total_active > 0 && { Alerts: { count: alertSummary.total_active } }),
  };

  const breadcrumb = deviceDetailMatch ? (
    <>
      <span className="breadcrumb-link" onClick={() => navigate('/devices')}>
        Devices
      </span>
      <span className="breadcrumb-sep"> / </span>
      <span>{deviceData?.name ?? deviceId}</span>
    </>
  ) : (
    currentRoute
  );

  return (
    <AppShell
      productName="Extrittio"
      collapsedProductName="Ex"
      navGroups={navGroups}
      navBadges={navBadges}
      projects={projects}
      user={currentUser}
      version="v0.1.0"
      sidebarCollapsed={sidebarCollapsed}
      onToggleSidebar={toggleSidebar}
      onOpenCommandPalette={openCommandPalette}
      onLogout={logout}
      breadcrumb={breadcrumb}
      commandPalette={<CommandPalette />}
    >
      {children}
    </AppShell>
  );
};
