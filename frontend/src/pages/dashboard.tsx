import { Card, Elevation, H3, H5, Icon } from '@blueprintjs/core';
import type { IconName } from '@blueprintjs/icons';
import './dashboard.css';

export const Dashboard = () => {
  const stats: { label: string; value: string; icon: IconName; color: string }[] = [
    { label: 'Total Devices', value: '1,234', icon: 'mobile-video', color: '#2965CC' },
    { label: 'Active Devices', value: '987', icon: 'tick-circle', color: '#0F9960' },
    { label: 'Offline Devices', value: '247', icon: 'warning-sign', color: '#D99E0B' },
    { label: 'Total Messages', value: '45.2K', icon: 'envelope', color: '#8F398F' },
  ];

  return (
    <div className="dashboard-page">
      <div className="page-header">
        <H3>Dashboard</H3>
        <p className="page-description">Welcome to Extrittio IoT Hub</p>
      </div>

      <div className="stats-grid">
        {stats.map((stat) => (
          <Card key={stat.label} elevation={Elevation.TWO} className="stat-card">
            <div className="stat-icon" style={{ backgroundColor: stat.color }}>
              <Icon icon={stat.icon} size={24} color="white" />
            </div>
            <div className="stat-content">
              <H5 className="stat-value">{stat.value}</H5>
              <p className="stat-label">{stat.label}</p>
            </div>
          </Card>
        ))}
      </div>

      <div className="dashboard-content">
        <div className="content-grid">
          <Card elevation={Elevation.TWO} className="content-card">
            <H5>Recent Activity</H5>
            <p className="text-muted">No recent activity to display</p>
          </Card>

          <Card elevation={Elevation.TWO} className="content-card">
            <H5>Device Status</H5>
            <p className="text-muted">Device statistics will appear here</p>
          </Card>
        </div>
      </div>
    </div>
  );
};
