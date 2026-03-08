import {
  Smartphone,
  CheckCircle2,
  AlertTriangle,
  Mail,
  TrendingUp,
  TrendingDown,
  type LucideIcon,
} from 'lucide-react';
import {
  AreaChart,
  Area,
  PieChart,
  Pie,
  Cell,
  ResponsiveContainer,
} from 'recharts';
import { Card } from '@/components/ui/card';

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
  icon: LucideIcon;
  color: string;
  sparkIndex: number;
}

const stats: StatCard[] = [
  { label: 'Total Devices', value: '1,234', delta: '+18 this week', deltaUp: true, icon: Smartphone, color: '#2965CC', sparkIndex: 0 },
  { label: 'Active Devices', value: '987', delta: '+12 today', deltaUp: true, icon: CheckCircle2, color: '#0F9960', sparkIndex: 1 },
  { label: 'Offline Devices', value: '247', delta: '-5 from yesterday', deltaUp: false, icon: AlertTriangle, color: '#D99E0B', sparkIndex: 2 },
  { label: 'Total Messages', value: '45.2K', delta: '+2.1K today', deltaUp: true, icon: Mail, color: '#8F398F', sparkIndex: 3 },
];

const donutData = [
  { name: 'Online', value: 987, color: 'hsl(152, 69%, 45%)' },
  { name: 'Offline', value: 247, color: 'hsl(0, 84%, 60%)' },
  { name: 'Warning', value: 23, color: 'hsl(38, 92%, 55%)' },
];

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
  const totalDevices = donutData.reduce((sum, d) => sum + d.value, 0);

  return (
    <div className="mx-auto max-w-[1400px] space-y-6">
      {/* System Health Strip */}
      <div className="flex flex-wrap items-center overflow-hidden rounded-md border border-border bg-surface">
        {healthMetrics.map((metric) => (
          <div
            key={metric.label}
            className="flex flex-1 items-center gap-2 border-r border-border px-5 py-2.5 last:border-r-0 max-md:flex-[1_1_45%] max-md:border-b max-md:border-border"
          >
            <span className={`status-led status-led--${metric.status}`} />
            <span className="text-[10px] font-bold tracking-[0.08em] text-muted">{metric.label}</span>
            <span className="font-mono tabular-nums font-semibold tracking-wide text-[13px] text-foreground">{metric.value}</span>
          </div>
        ))}
      </div>

      {/* Page Header */}
      <div>
        <h3 className="text-lg font-semibold tracking-tight text-foreground">Dashboard</h3>
        <p className="mt-1 text-sm text-muted">Extrittio IoT Hub — Operational Overview</p>
      </div>

      {/* Stat Cards */}
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
        {stats.map((stat) => (
          <Card
            key={stat.label}
            className="stagger-item border-l-3 p-4"
            style={{ borderLeftColor: stat.color }}
          >
            <div className="mb-3 flex items-center justify-between">
              <div
                className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg"
                style={{ backgroundColor: stat.color }}
              >
                <stat.icon size={20} color="white" />
              </div>
              <div className="h-8 max-w-[100px] flex-1">
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
            <div className="flex flex-col gap-0.5">
              <span className="text-[28px] font-bold leading-none text-foreground font-mono tabular-nums">{stat.value}</span>
              <span className="text-xs font-semibold uppercase tracking-[0.06em] text-muted">{stat.label}</span>
              <span className={`mt-1 flex items-center gap-1 text-xs font-semibold ${stat.deltaUp ? 'text-success' : 'text-danger'}`}>
                {stat.deltaUp ? <TrendingUp size={12} /> : <TrendingDown size={12} />}
                {stat.delta}
              </span>
            </div>
          </Card>
        ))}
      </div>

      {/* Content Grid */}
      <div className="grid grid-cols-1 gap-4 lg:grid-cols-[1.4fr_1fr]">
        {/* Activity Timeline */}
        <Card className="stagger-item p-5">
          <div className="mb-4 flex items-center justify-between">
            <h5 className="text-sm font-semibold text-foreground">Recent Activity</h5>
            <span className="text-[11px] font-bold uppercase tracking-widest text-muted">{activityEvents.length} events</span>
          </div>
          <div className="flex flex-col">
            {activityEvents.map((event) => (
              <div
                key={event.id}
                className="flex items-center gap-2.5 border-b border-[hsl(0_0%_15%/0.5)] py-2 text-[13px] last:border-b-0"
              >
                <span className={`status-led status-led--${event.status}`} />
                <span className="min-w-[60px] shrink-0 font-mono tabular-nums font-semibold tracking-wide text-[11px] text-muted">{event.time}</span>
                <span className="min-w-0 shrink overflow-hidden text-ellipsis whitespace-nowrap font-semibold text-foreground">{event.device}</span>
                <span className="ml-auto shrink-0 overflow-hidden text-ellipsis whitespace-nowrap text-muted">{event.event}</span>
              </div>
            ))}
          </div>
        </Card>

        {/* Device Status Donut */}
        <Card className="stagger-item p-5">
          <div className="mb-4 flex items-center justify-between">
            <h5 className="text-sm font-semibold text-foreground">Device Status</h5>
            <span className="text-[11px] font-bold uppercase tracking-widest text-muted">{totalDevices} total</span>
          </div>
          <div className="relative flex justify-center py-2.5">
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
            <div className="pointer-events-none absolute left-1/2 top-1/2 flex -translate-x-1/2 -translate-y-1/2 flex-col items-center">
              <span className="text-2xl font-bold leading-none text-foreground font-mono tabular-nums">{totalDevices.toLocaleString()}</span>
              <span className="mt-0.5 text-[11px] font-semibold uppercase tracking-[0.06em] text-muted">Devices</span>
            </div>
          </div>
          <div className="mt-2 flex justify-center gap-6">
            {donutData.map((entry) => (
              <div key={entry.name} className="flex items-center gap-1.5 text-[13px]">
                <span className="h-2 w-2 shrink-0 rounded-full" style={{ backgroundColor: entry.color }} />
                <span className="font-medium text-muted">{entry.name}</span>
                <span className="font-mono tabular-nums font-semibold tracking-wide text-foreground">{entry.value}</span>
              </div>
            ))}
          </div>
        </Card>
      </div>
    </div>
  );
};
