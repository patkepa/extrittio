import { Card, Elevation, H3, H5, Icon } from '@blueprintjs/core';
import type { IconName } from '@blueprintjs/icons';
import {
  AreaChart,
  Area,
  PieChart,
  Pie,
  Cell,
  ResponsiveContainer,
} from 'recharts';
import { useDashboardStats } from '../hooks/use-dashboard';
import './dashboard.css';

const sparklineData = [
  [40, 45, 42, 50, 55, 52, 58],
  [30, 35, 38, 40, 42, 44, 47],
  [15, 12, 18, 14, 10, 13, 12],
  [20, 25, 28, 30, 35, 40, 45],
];

interface StatCard {
  label: string;
  value: string;
  delta: string;
  deltaUp: boolean;
  icon: IconName;
  color: string;
  sparkIndex: number;
}

function formatCount(n: number): string {
  if (n >= 1000) return `${(n / 1000).toFixed(1)}K`;
  return n.toString();
}

interface ActivityEvent {
  id: string;
  time: string;
  device: string;
  event: string;
  status: 'online' | 'offline' | 'warning';
}

const activityEvents: ActivityEvent[] = [
  { id: '1', time: '14:32:01', device: 'Temperature Sensor 01', event: 'Came online', status: 'online' },
  { id: '2', time: '14:28:45', device: 'Smart Lock 08', event: 'Firmware updated to v3.0.1', status: 'online' },
  { id: '3', time: '14:15:22', device: 'Motion Detector 12', event: 'Went offline', status: 'offline' },
  { id: '4', time: '13:58:03', device: 'Humidity Sensor 05', event: 'High humidity alert', status: 'warning' },
  { id: '5', time: '13:42:17', device: 'Smart Camera 03', event: 'Came online', status: 'online' },
  { id: '6', time: '13:30:00', device: 'Temperature Sensor 04', event: 'Battery low warning', status: 'warning' },
  { id: '7', time: '13:15:44', device: 'Smart Lock 02', event: 'Came online', status: 'online' },
  { id: '8', time: '12:58:12', device: 'Motion Detector 07', event: 'Went offline', status: 'offline' },
];

const healthMetrics = [
  { label: 'UPTIME', value: '99.97%', status: 'online' as const },
  { label: 'API LATENCY', value: '23ms', status: 'online' as const },
  { label: 'LAST SYNC', value: '14:32:01', status: 'online' as const },
  { label: 'CONNECTIONS', value: '987', status: 'online' as const },
];

export const Dashboard = () => {
  const { data: dashboardStats } = useDashboardStats();

  const stats: StatCard[] = dashboardStats
    ? [
        { label: 'Total Devices', value: formatCount(dashboardStats.total_devices), delta: '', deltaUp: true, icon: 'mobile-video', color: '#2965CC', sparkIndex: 0 },
        { label: 'Active Devices', value: formatCount(dashboardStats.active_devices), delta: '', deltaUp: true, icon: 'tick-circle', color: '#0F9960', sparkIndex: 1 },
        { label: 'Offline Devices', value: formatCount(dashboardStats.offline_devices), delta: '', deltaUp: false, icon: 'warning-sign', color: '#D99E0B', sparkIndex: 2 },
        { label: 'Total Messages', value: formatCount(dashboardStats.total_messages), delta: '', deltaUp: true, icon: 'envelope', color: '#8F398F', sparkIndex: 3 },
      ]
    : [
        { label: 'Total Devices', value: '\u2014', delta: '', deltaUp: true, icon: 'mobile-video', color: '#2965CC', sparkIndex: 0 },
        { label: 'Active Devices', value: '\u2014', delta: '', deltaUp: true, icon: 'tick-circle', color: '#0F9960', sparkIndex: 1 },
        { label: 'Offline Devices', value: '\u2014', delta: '', deltaUp: false, icon: 'warning-sign', color: '#D99E0B', sparkIndex: 2 },
        { label: 'Total Messages', value: '\u2014', delta: '', deltaUp: true, icon: 'envelope', color: '#8F398F', sparkIndex: 3 },
      ];

  const donutData = dashboardStats
    ? [
        { name: 'Online', value: dashboardStats.active_devices, color: 'hsl(152, 69%, 45%)' },
        { name: 'Offline', value: dashboardStats.offline_devices, color: 'hsl(0, 84%, 60%)' },
      ]
    : [
        { name: 'Online', value: 0, color: 'hsl(152, 69%, 45%)' },
        { name: 'Offline', value: 0, color: 'hsl(0, 84%, 60%)' },
      ];

  const totalDevices = donutData.reduce((sum, d) => sum + d.value, 0);

  return (
    <div className="dashboard-page">
      {/* System Health Strip */}
      <div className="health-strip">
        {healthMetrics.map((metric) => (
          <div key={metric.label} className="health-metric">
            <span className={`status-led status-led--${metric.status}`} />
            <span className="health-label">{metric.label}</span>
            <span className="health-value mono-data">{metric.value}</span>
          </div>
        ))}
      </div>

      <div className="page-header">
        <H3>Dashboard</H3>
        <p className="page-description">Extrittio IoT Hub — Operational Overview</p>
      </div>

      {/* Stat Cards */}
      <div className="stats-grid">
        {stats.map((stat) => (
          <Card key={stat.label} elevation={Elevation.TWO} className="stat-card stagger-item">
            <div className="stat-card-top">
              <div className="stat-icon" style={{ backgroundColor: stat.color }}>
                <Icon icon={stat.icon} size={20} color="white" />
              </div>
              <div className="stat-sparkline">
                <ResponsiveContainer width="100%" height={32}>
                  <AreaChart data={sparklineData[stat.sparkIndex]!.map((v, i) => ({ v, i }))}>
                    <defs>
                      <linearGradient id={`spark-${stat.sparkIndex}`} x1="0" y1="0" x2="0" y2="1">
                        <stop offset="0%" stopColor={stat.color} stopOpacity={0.3} />
                        <stop offset="100%" stopColor={stat.color} stopOpacity={0} />
                      </linearGradient>
                    </defs>
                    <Area
                      type="monotone"
                      dataKey="v"
                      stroke={stat.color}
                      strokeWidth={1.5}
                      fill={`url(#spark-${stat.sparkIndex})`}
                      dot={false}
                      isAnimationActive={false}
                    />
                  </AreaChart>
                </ResponsiveContainer>
              </div>
            </div>
            <div className="stat-content">
              <span className="stat-value mono-data">{stat.value}</span>
              <span className="stat-label">{stat.label}</span>
              <span className={`stat-delta ${stat.deltaUp ? 'delta-up' : 'delta-down'}`}>
                <Icon icon={stat.deltaUp ? 'trending-up' : 'trending-down'} size={12} />
                {stat.delta}
              </span>
            </div>
          </Card>
        ))}
      </div>

      {/* Content Grid */}
      <div className="dashboard-content">
        <div className="content-grid">
          {/* Activity Timeline */}
          <Card elevation={Elevation.TWO} className="content-card stagger-item">
            <div className="card-header">
              <H5>Recent Activity</H5>
              <span className="section-label" style={{ margin: 0 }}>{activityEvents.length} events</span>
            </div>
            <div className="activity-timeline">
              {activityEvents.map((event) => (
                <div key={event.id} className="timeline-item">
                  <span className={`status-led status-led--${event.status}`} />
                  <span className="timeline-time mono-data">{event.time}</span>
                  <span className="timeline-device">{event.device}</span>
                  <span className="timeline-event">{event.event}</span>
                </div>
              ))}
            </div>
          </Card>

          {/* Device Status Donut */}
          <Card elevation={Elevation.TWO} className="content-card stagger-item">
            <div className="card-header">
              <H5>Device Status</H5>
              <span className="section-label" style={{ margin: 0 }}>{totalDevices} total</span>
            </div>
            <div className="donut-container">
              <ResponsiveContainer width="100%" height={200}>
                <PieChart>
                  <Pie
                    data={donutData}
                    cx="50%"
                    cy="50%"
                    innerRadius={60}
                    outerRadius={85}
                    paddingAngle={3}
                    dataKey="value"
                    strokeWidth={0}
                    isAnimationActive={false}
                  >
                    {donutData.map((entry) => (
                      <Cell key={entry.name} fill={entry.color} />
                    ))}
                  </Pie>
                </PieChart>
              </ResponsiveContainer>
              <div className="donut-center">
                <span className="donut-total mono-data">{totalDevices.toLocaleString()}</span>
                <span className="donut-label">Devices</span>
              </div>
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
        </div>
      </div>
    </div>
  );
};
