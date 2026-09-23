import { Button, Callout, Card, Elevation, H5, Icon, Spinner } from '@blueprintjs/core';
import type { IconName } from '@blueprintjs/icons';
import { Link } from 'react-router-dom';
import { SVGDonut } from '../components/charts/SVGDonut';
import { useDashboardStats } from '../hooks/use-dashboard';
import { useAlertSummary } from '../features/alerts/queries/use-alerts';
import { useActivityEvents } from '../features/activity/queries/use-activity-events';
import { ServerHealth } from '../components/dashboard/server-health';
import { hasPermission } from '../auth/permissions';
import { useAuthStore } from '../stores/auth-store';
import './dashboard.css';

export const Dashboard = () => {
  const permissions = useAuthStore((state) => state.user?.permissions);
  const canReadAlerts = hasPermission(permissions, 'alerts.read');
  const canReadLogs = hasPermission(permissions, 'logs.read');
  const canReadServerMetrics = hasPermission(permissions, 'server_metrics.read');
  const statsQuery = useDashboardStats();
  const alertsQuery = useAlertSummary({ enabled: canReadAlerts });
  const activityQuery = useActivityEvents(
    { limit: 8, offset: 0 },
    { enabled: canReadLogs, refetchInterval: 30_000 },
  );
  const stats = statsQuery.data;

  const cards: { label: string; value: number; icon: IconName; color: string }[] = stats
    ? [
        {
          label: 'Total Devices',
          value: stats.total_devices,
          icon: 'mobile-video',
          color: '#2965CC',
        },
        {
          label: 'Active Devices',
          value: stats.active_devices,
          icon: 'tick-circle',
          color: '#0F9960',
        },
        {
          label: 'Offline Devices',
          value: stats.offline_devices,
          icon: 'warning-sign',
          color: '#D99E0B',
        },
        {
          label: 'Total Messages',
          value: stats.total_messages,
          icon: 'envelope',
          color: '#8F398F',
        },
      ]
    : [];
  if (canReadAlerts && alertsQuery.data) {
    cards.push({
      label: 'Active Alerts',
      value: alertsQuery.data.total_active,
      icon: 'notifications',
      color: '#DB3737',
    });
  }
  const donutData = stats
    ? [
        { name: 'Online', value: stats.active_devices, color: 'hsl(152, 69%, 45%)' },
        { name: 'Offline', value: stats.offline_devices, color: 'hsl(0, 84%, 60%)' },
      ]
    : [];

  return (
    <div className="dashboard-page">
      {canReadServerMetrics && <ServerHealth />}
      <div className="dashboard-section">
        <H5>Devices</H5>
        {statsQuery.isPending && (
          <div className="dashboard-loading">
            <Spinner /> Loading dashboard…
          </div>
        )}
        {statsQuery.isError && (
          <Callout intent="danger" icon="error" className="dashboard-error">
            Could not load device statistics.{' '}
            <Button minimal small icon="refresh" onClick={() => void statsQuery.refetch()}>
              Retry
            </Button>
          </Callout>
        )}
        {stats && (
          <div className="stats-grid">
            {cards.map((card) => (
              <Card key={card.label} elevation={Elevation.ONE} className="stat-card stagger-item">
                <div className="stat-card-top">
                  <div className="stat-icon" style={{ backgroundColor: card.color }}>
                    <Icon icon={card.icon} size={20} color="white" />
                  </div>
                </div>
                <div className="stat-content">
                  <span className="stat-value mono-data">{card.value.toLocaleString()}</span>
                  <span className="stat-label">{card.label}</span>
                </div>
              </Card>
            ))}
          </div>
        )}
        <div className={canReadLogs ? 'content-grid' : 'content-grid content-grid--single'}>
          {canReadLogs && (
            <Card elevation={Elevation.ONE} className="content-card stagger-item">
              <div className="card-header">
                <H5>Recent Activity</H5>
                <Link to="/logs">View logs</Link>
              </div>
              {activityQuery.isPending && <Spinner size={20} />}
              {activityQuery.isError && (
                <Callout intent="danger" icon="error">
                  Could not load activity.{' '}
                  <Button minimal small icon="refresh" onClick={() => void activityQuery.refetch()}>
                    Retry
                  </Button>
                </Callout>
              )}
              {activityQuery.data && (
                <div className="activity-timeline">
                  {activityQuery.data.data.length === 0 && <p>No activity yet.</p>}
                  {activityQuery.data.data.map((event) => (
                    <div key={event.id} className="timeline-item">
                      <span
                        className="timeline-time mono-data"
                        title={new Date(event.occurred_at).toLocaleString()}
                      >
                        {new Date(event.occurred_at).toLocaleTimeString()}
                      </span>
                      <span className="timeline-device">{event.source}</span>
                      <span className="timeline-event" title={event.message}>
                        {event.message}
                      </span>
                    </div>
                  ))}
                </div>
              )}
            </Card>
          )}
          {stats && (
            <Card elevation={Elevation.ONE} className="content-card stagger-item">
              <div className="card-header">
                <H5>Device Status</H5>
                <span className="section-label">{stats.total_devices} total</span>
              </div>
              <div className="donut-container">
                <SVGDonut segments={donutData}>
                  <span className="donut-total mono-data">
                    {stats.total_devices.toLocaleString()}
                  </span>
                  <span className="donut-label">Devices</span>
                </SVGDonut>
              </div>
              <div className="donut-legend">
                {donutData.map((entry) => (
                  <div key={entry.name} className="legend-item">
                    <span className="legend-dot" style={{ backgroundColor: entry.color }} />
                    <span className="legend-name">{entry.name}</span>
                    <span className="legend-value mono-data">{entry.value}</span>
                  </div>
                ))}
              </div>
            </Card>
          )}
        </div>
      </div>
    </div>
  );
};
