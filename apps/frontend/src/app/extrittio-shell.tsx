import { useEffect, type ReactNode } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';
import { CommandPalette } from '../components/command-palette/command-palette';
import { UserSettingsMenu } from '../components/layout/user-settings-menu';
import { hasPermission } from '../auth/permissions';
import { useAlertSummary } from '../features/alerts/queries/use-alerts';
import { useDashboardStats } from '../hooks/use-dashboard';
import { useDevice } from '../hooks/use-devices';
import { WorkspacePortal, WorkspaceShell } from '@patkepa/kantzen-ui/app-shell';
import type { NavBadge } from '@patkepa/kantzen-ui/navigation';
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
    <WorkspaceShell
      productName="Extrittio"
      collapsedProductName="Ex"
      currentPath={location.pathname}
      navGroups={navGroups}
      navBadges={navBadges}
      sidebarCollapsed={sidebarCollapsed}
      onToggleSidebar={toggleSidebar}
      onNavigate={navigate}
      onOpenCommandPalette={openCommandPalette}
      breadcrumb={breadcrumb}
      commandPalette={<CommandPalette />}
    >
      <WorkspacePortal slot="sidebar-nav-end">
        <UserSettingsMenu
          user={currentUser}
          collapsed={sidebarCollapsed}
          onNavigate={navigate}
          onLogout={logout}
        />
      </WorkspacePortal>
      {children}
    </WorkspaceShell>
  );
};
