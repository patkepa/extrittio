import { useEffect, type ReactNode } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';
import { CommandPalette } from '../components/command-palette/command-palette';
import { projects } from '../data/sidebar-data';
import { hasPermission } from '../auth/permissions';
import { useAlertSummary } from '../features/alerts';
import { useDashboardStats } from '../hooks/use-dashboard';
import { useDevice } from '../hooks/use-devices';
import { AppShell } from '@extrittio/app-shell';
import type { NavBadge } from '@extrittio/navigation';
import { useAuthStore } from '../stores/auth-store';
import { useUIStore } from '../stores/ui-store';
import { getNavGroups, getRouteLabel } from './routes';

interface ExtrittioShellProps {
  children: ReactNode;
}

export const ExtrittioShell = ({ children }: ExtrittioShellProps) => {
  const location = useLocation();
  const navigate = useNavigate();
  const authUser = useAuthStore((s) => s.user);
  const permissions = authUser?.permissions;
  const logout = useAuthStore((s) => s.logout);
  const openCommandPalette = useUIStore((s) => s.openCommandPalette);
  const sidebarCollapsed = useUIStore((s) => s.isSidebarCollapsed);
  const toggleSidebar = useUIStore((s) => s.toggleSidebar);
  const toggleContextPanel = useUIStore((s) => s.toggleContextPanel);
  const canReadDevices = hasPermission(permissions, 'devices.read');
  const canReadAlerts = hasPermission(permissions, 'alerts.read');
  const { data: dashboardStats } = useDashboardStats({ enabled: canReadDevices });
  const { data: alertSummary } = useAlertSummary({ enabled: canReadAlerts });

  const deviceDetailMatch = location.pathname.match(/^\/devices\/([^/]+)$/);
  const deviceId = deviceDetailMatch?.[1] ?? null;
  const { data: deviceData } = useDevice(canReadDevices ? deviceId : null);
  const currentRoute = deviceDetailMatch ? null : getRouteLabel(location.pathname, permissions);
  const navGroups = getNavGroups(permissions);
  const currentUser = {
    name: authUser?.username ?? 'User',
    email: authUser?.roles?.map((role) => role.name).join(', ') || authUser?.role || '',
  };

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

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.shiftKey && event.key.toLowerCase() === 'b') {
        event.preventDefault();
        toggleContextPanel();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [toggleContextPanel]);

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
