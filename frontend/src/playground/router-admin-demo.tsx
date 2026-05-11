import React, { useMemo, useState } from 'react';
import ReactDOM from 'react-dom/client';
import { HashRouter, Route, Routes, useLocation, useNavigate } from 'react-router-dom';
import {
  Button,
  Card,
  Elevation,
  HTMLSelect,
  Icon,
  Intent,
  ProgressBar,
  Switch,
  Tag,
} from '@blueprintjs/core';
import type { IconName } from '@blueprintjs/icons';
import { Command } from 'cmdk';
import { AppShell } from '@patkepa/app-shell';
import { CommandPaletteShell } from '@patkepa/command-palette';
import { createApiClient } from '@patkepa/data-client';
import type { NavGroup, Project, User } from '@patkepa/navigation';
import { ThemeProvider } from '@patkepa/theme';
import { EmptyState, FilterPill, MainToolbar, SearchField, StatusLed } from '@patkepa/ui';

import '@blueprintjs/core/lib/css/blueprint.css';
import '@blueprintjs/icons/lib/css/blueprint-icons.css';
import '@patkepa/theme/theme.css';
import './router-admin-demo.css';

type RouterStatus = 'online' | 'warning' | 'offline';
type InterfaceType = 'WAN' | 'LAN' | 'VLAN' | 'VPN';
type LogLevel = 'info' | 'warning' | 'critical';
type FirewallAction = 'allow' | 'deny';

interface RouterInterface {
  id: string;
  name: string;
  status: RouterStatus;
  type: InterfaceType;
  address: string;
  mac: string;
  rxMbps: number;
  txMbps: number;
  errors: number;
  uptime: string;
}

interface ConnectedClient {
  id: string;
  hostname: string;
  ip: string;
  mac: string;
  network: string;
  signal: number;
  traffic: string;
  lastSeen: string;
  status: RouterStatus;
  blocked: boolean;
}

interface WifiNetwork {
  id: string;
  ssid: string;
  band: '2.4 GHz' | '5 GHz' | '6 GHz';
  channel: string;
  security: string;
  clients: number;
  utilization: number;
  enabled: boolean;
  hidden: boolean;
}

interface RouteEntry {
  id: string;
  destination: string;
  gateway: string;
  iface: string;
  metric: number;
  type: 'static' | 'connected' | 'policy';
  status: RouterStatus;
}

interface FirewallRule {
  id: string;
  name: string;
  source: string;
  destination: string;
  service: string;
  action: FirewallAction;
  hits: number;
  enabled: boolean;
}

interface LogEntry {
  id: string;
  time: string;
  level: LogLevel;
  source: string;
  message: string;
}

const navGroups: NavGroup[] = [
  {
    label: 'Router',
    items: [
      { label: 'Overview', icon: 'dashboard', href: '/' },
      { label: 'Interfaces', icon: 'exchange', href: '/interfaces' },
      { label: 'Clients', icon: 'people', href: '/clients' },
      { label: 'Wireless', icon: 'antenna', href: '/wireless' },
    ],
  },
  {
    label: 'Network',
    items: [
      { label: 'Routes', icon: 'route', href: '/routes' },
      { label: 'Firewall', icon: 'shield', href: '/firewall' },
      { label: 'Logs', icon: 'manual', href: '/logs' },
      { label: 'Maintenance', icon: 'wrench', href: '/maintenance' },
    ],
  },
];

const projects: Project[] = [
  { name: 'Edge Router', environment: 'Lab', icon: 'lab-test', color: 'hsl(var(--accent))' },
  { name: 'Edge Router', environment: 'Staging', icon: 'build', color: 'hsl(var(--warning))' },
  {
    name: 'Edge Router',
    environment: 'Production',
    icon: 'satellite',
    color: 'hsl(var(--success))',
  },
];

const user: User = {
  name: 'Network Operator',
  email: 'netops@example.internal',
};

const interfaces: RouterInterface[] = [
  {
    id: 'wan0',
    name: 'wan0',
    status: 'online',
    type: 'WAN',
    address: '198.51.100.14/30',
    mac: 'A4:7B:2C:10:44:01',
    rxMbps: 814,
    txMbps: 196,
    errors: 0,
    uptime: '42d 18h',
  },
  {
    id: 'lan0',
    name: 'lan0',
    status: 'online',
    type: 'LAN',
    address: '10.42.0.1/24',
    mac: 'A4:7B:2C:10:44:02',
    rxMbps: 2100,
    txMbps: 1840,
    errors: 0,
    uptime: '42d 18h',
  },
  {
    id: 'iot-vlan',
    name: 'iot-vlan',
    status: 'online',
    type: 'VLAN',
    address: '10.42.30.1/24',
    mac: 'A4:7B:2C:10:44:03',
    rxMbps: 148,
    txMbps: 92,
    errors: 2,
    uptime: '29d 6h',
  },
  {
    id: 'vpn0',
    name: 'vpn0',
    status: 'warning',
    type: 'VPN',
    address: '172.18.6.1/24',
    mac: 'A4:7B:2C:10:44:04',
    rxMbps: 164,
    txMbps: 88,
    errors: 12,
    uptime: '6d 3h',
  },
  {
    id: 'backup-wan',
    name: 'backup-wan',
    status: 'offline',
    type: 'WAN',
    address: '203.0.113.9/30',
    mac: 'A4:7B:2C:10:44:05',
    rxMbps: 0,
    txMbps: 0,
    errors: 0,
    uptime: 'disabled',
  },
];

const initialClients: ConnectedClient[] = [
  {
    id: 'ops-laptop',
    hostname: 'ops-laptop-14',
    ip: '10.42.0.24',
    mac: '68:5B:35:84:2D:18',
    network: 'LAN',
    signal: 100,
    traffic: '18.4 GB',
    lastSeen: 'now',
    status: 'online',
    blocked: false,
  },
  {
    id: 'nas-array',
    hostname: 'storage-array-a',
    ip: '10.42.0.8',
    mac: '80:61:5F:9C:B1:22',
    network: 'LAN',
    signal: 100,
    traffic: '441.2 GB',
    lastSeen: 'now',
    status: 'online',
    blocked: false,
  },
  {
    id: 'camera-01',
    hostname: 'dock-camera-01',
    ip: '10.42.30.44',
    mac: '9C:7D:A3:01:1C:48',
    network: 'IoT',
    signal: 72,
    traffic: '6.8 GB',
    lastSeen: '2 min',
    status: 'online',
    blocked: false,
  },
  {
    id: 'badge-reader',
    hostname: 'badge-reader-west',
    ip: '10.42.30.51',
    mac: 'A0:EE:1B:74:AC:11',
    network: 'IoT',
    signal: 48,
    traffic: '248 MB',
    lastSeen: '9 min',
    status: 'warning',
    blocked: false,
  },
  {
    id: 'guest-phone',
    hostname: 'guest-phone-22',
    ip: '10.42.80.31',
    mac: '02:10:44:8C:E2:09',
    network: 'Guest',
    signal: 68,
    traffic: '911 MB',
    lastSeen: '14 min',
    status: 'online',
    blocked: false,
  },
  {
    id: 'unknown',
    hostname: 'unknown-device',
    ip: '10.42.80.79',
    mac: 'F2:93:20:AA:4C:0D',
    network: 'Guest',
    signal: 33,
    traffic: '12 MB',
    lastSeen: '1h',
    status: 'offline',
    blocked: true,
  },
];

const initialWifiNetworks: WifiNetwork[] = [
  {
    id: 'ops',
    ssid: 'Ops-Control',
    band: '6 GHz',
    channel: '37',
    security: 'WPA3 Enterprise',
    clients: 18,
    utilization: 0.48,
    enabled: true,
    hidden: false,
  },
  {
    id: 'iot',
    ssid: 'IoT-Sensors',
    band: '2.4 GHz',
    channel: '11',
    security: 'WPA2 PSK',
    clients: 43,
    utilization: 0.72,
    enabled: true,
    hidden: true,
  },
  {
    id: 'guest',
    ssid: 'Guest-Access',
    band: '5 GHz',
    channel: '149',
    security: 'Captive Portal',
    clients: 7,
    utilization: 0.22,
    enabled: true,
    hidden: false,
  },
];

const routeEntries: RouteEntry[] = [
  {
    id: 'default',
    destination: '0.0.0.0/0',
    gateway: '198.51.100.13',
    iface: 'wan0',
    metric: 10,
    type: 'static',
    status: 'online',
  },
  {
    id: 'lan',
    destination: '10.42.0.0/24',
    gateway: 'connected',
    iface: 'lan0',
    metric: 0,
    type: 'connected',
    status: 'online',
  },
  {
    id: 'iot',
    destination: '10.42.30.0/24',
    gateway: 'connected',
    iface: 'iot-vlan',
    metric: 0,
    type: 'connected',
    status: 'online',
  },
  {
    id: 'vpn',
    destination: '172.18.0.0/16',
    gateway: '172.18.6.254',
    iface: 'vpn0',
    metric: 80,
    type: 'policy',
    status: 'warning',
  },
  {
    id: 'backup',
    destination: '0.0.0.0/0',
    gateway: '203.0.113.10',
    iface: 'backup-wan',
    metric: 250,
    type: 'static',
    status: 'offline',
  },
];

const initialFirewallRules: FirewallRule[] = [
  {
    id: 'allow-lan-out',
    name: 'LAN outbound internet',
    source: 'LAN',
    destination: 'WAN',
    service: 'Any TCP/UDP',
    action: 'allow',
    hits: 148204,
    enabled: true,
  },
  {
    id: 'deny-guest-lan',
    name: 'Guest isolation',
    source: 'Guest',
    destination: 'LAN',
    service: 'Any',
    action: 'deny',
    hits: 884,
    enabled: true,
  },
  {
    id: 'allow-vpn-admin',
    name: 'VPN admin access',
    source: 'VPN',
    destination: 'Router',
    service: 'HTTPS, SSH',
    action: 'allow',
    hits: 351,
    enabled: true,
  },
  {
    id: 'deny-iot-wan-unknown',
    name: 'IoT unknown egress',
    source: 'IoT',
    destination: 'WAN',
    service: 'Non-approved',
    action: 'deny',
    hits: 12742,
    enabled: true,
  },
  {
    id: 'allow-maintenance',
    name: 'Maintenance window',
    source: 'LAN',
    destination: 'IoT',
    service: 'SSH',
    action: 'allow',
    hits: 18,
    enabled: false,
  },
];

const logs: LogEntry[] = [
  {
    id: 'log-1',
    time: '21:07:14',
    level: 'warning',
    source: 'vpn0',
    message: 'Tunnel latency exceeded 90 ms threshold for 3 consecutive samples',
  },
  {
    id: 'log-2',
    time: '21:06:52',
    level: 'info',
    source: 'dhcp',
    message: 'Lease renewed for dock-camera-01 at 10.42.30.44',
  },
  {
    id: 'log-3',
    time: '21:05:33',
    level: 'critical',
    source: 'firewall',
    message: 'Blocked guest network attempt to reach 10.42.0.8 on TCP/445',
  },
  {
    id: 'log-4',
    time: '21:04:18',
    level: 'info',
    source: 'wan0',
    message: 'Carrier stable at 2.5G full duplex',
  },
  {
    id: 'log-5',
    time: '21:02:01',
    level: 'warning',
    source: 'wireless',
    message: 'IoT-Sensors channel utilization above 70 percent',
  },
  {
    id: 'log-6',
    time: '20:59:44',
    level: 'info',
    source: 'system',
    message: 'Configuration snapshot saved to local retention store',
  },
];

const throughputSamples = [
  { time: '20:42', download: 318, upload: 74 },
  { time: '20:45', download: 446, upload: 92 },
  { time: '20:48', download: 610, upload: 141 },
  { time: '20:51', download: 530, upload: 118 },
  { time: '20:54', download: 812, upload: 192 },
  { time: '20:57', download: 740, upload: 165 },
  { time: '21:00', download: 905, upload: 204 },
  { time: '21:03', download: 814, upload: 196 },
];

const routerAdminApi = createApiClient({
  baseUrl: '/router-admin-demo/api',
  getToken: () => 'demo-token',
});

void routerAdminApi;

function statusLabel(status: RouterStatus) {
  if (status === 'online') return 'Online';
  if (status === 'warning') return 'Warning';
  return 'Offline';
}

function logIntent(level: LogLevel): Intent {
  if (level === 'info') return Intent.SUCCESS;
  if (level === 'warning') return Intent.WARNING;
  return Intent.DANGER;
}

function countStatus(items: Array<{ status: RouterStatus }>, status: RouterStatus) {
  return items.filter((item) => item.status === status).length;
}

function formatMbps(value: number) {
  if (value >= 1000) return `${(value / 1000).toFixed(1)} Gbps`;
  return `${value} Mbps`;
}

function flattenNavGroups(groups: NavGroup[]) {
  return groups.flatMap((group) => group.items);
}

function PageToolbar({
  title,
  subtitle,
  children,
}: {
  title: string;
  subtitle: string;
  children?: React.ReactNode;
}) {
  return (
    <MainToolbar ariaLabel={`${title} toolbar`}>
      <div className="router-demo-toolbar">
        <div className="router-demo-title">
          <h2>{title}</h2>
          <p>{subtitle}</p>
        </div>
        {children && <div className="router-demo-actions">{children}</div>}
      </div>
    </MainToolbar>
  );
}

function Panel({
  title,
  icon,
  meta,
  children,
  className,
}: {
  title: string;
  icon?: IconName;
  meta?: React.ReactNode;
  children: React.ReactNode;
  className?: string;
}) {
  const classNames = ['router-demo-panel', className].filter(Boolean).join(' ');

  return (
    <Card elevation={Elevation.ONE} className={classNames}>
      <div className="router-demo-panel-header">
        <div className="router-demo-panel-title">
          {icon && <Icon icon={icon} size={16} />}
          <h3>{title}</h3>
        </div>
        {meta && <div className="router-demo-panel-meta">{meta}</div>}
      </div>
      {children}
    </Card>
  );
}

function MetricCard({
  label,
  value,
  meta,
  icon,
  status,
}: {
  label: string;
  value: string;
  meta: string;
  icon: IconName;
  status?: RouterStatus;
}) {
  return (
    <Card elevation={Elevation.ONE} className="router-demo-card">
      <div className="router-demo-card-topline">
        <Icon icon={icon} size={16} />
        {status && <StatusLed status={status} label={statusLabel(status)} />}
      </div>
      <span className="router-demo-label">{label}</span>
      <strong>{value}</strong>
      <span className="router-demo-muted">{meta}</span>
    </Card>
  );
}

function LogLevelTag({ level }: { level: LogLevel }) {
  return (
    <Tag minimal intent={logIntent(level)}>
      {level === 'critical' ? 'Critical' : level.charAt(0).toUpperCase() + level.slice(1)}
    </Tag>
  );
}

function UtilizationMeter({ value, label }: { value: number; label: string }) {
  const intent = value > 0.7 ? Intent.WARNING : Intent.SUCCESS;

  return (
    <div className="router-demo-meter">
      <div className="router-demo-meter-row">
        <span>{label}</span>
        <strong>{Math.round(value * 100)}%</strong>
      </div>
      <ProgressBar value={value} intent={intent} />
    </div>
  );
}

function ThroughputChart() {
  const maxThroughput = Math.max(...throughputSamples.map((sample) => sample.download));

  return (
    <div className="router-demo-chart" aria-label="WAN throughput samples">
      {throughputSamples.map((sample) => (
        <div key={sample.time} className="router-demo-chart-column">
          <div className="router-demo-chart-bars">
            <span
              className="router-demo-chart-bar router-demo-chart-bar--download"
              style={
                {
                  '--bar-height': `${Math.max((sample.download / maxThroughput) * 100, 8)}%`,
                } as React.CSSProperties
              }
            />
            <span
              className="router-demo-chart-bar router-demo-chart-bar--upload"
              style={
                {
                  '--bar-height': `${Math.max((sample.upload / maxThroughput) * 100, 6)}%`,
                } as React.CSSProperties
              }
            />
          </div>
          <span className="router-demo-chart-label mono-data">{sample.time}</span>
        </div>
      ))}
    </div>
  );
}

function InterfaceStatusLine({ item }: { item: RouterInterface }) {
  return (
    <div className="router-demo-interface-row">
      <StatusLed status={item.status} label={statusLabel(item.status)} />
      <strong>{item.name}</strong>
      <span>{item.type}</span>
      <span className="mono-data">{item.address}</span>
      <span className="mono-data">{formatMbps(item.rxMbps)}</span>
      <span className="mono-data">{formatMbps(item.txMbps)}</span>
    </div>
  );
}

function Overview({
  clients,
  wifiNetworks,
}: {
  clients: ConnectedClient[];
  wifiNetworks: WifiNetwork[];
}) {
  const onlineInterfaces = countStatus(interfaces, 'online');
  const warningInterfaces = countStatus(interfaces, 'warning');
  const totalClients = clients.filter((client) => !client.blocked).length;
  const wifiClients = wifiNetworks.reduce((sum, network) => sum + network.clients, 0);

  return (
    <div className="router-demo-page">
      <PageToolbar title="Edge Router" subtitle="Site 04 gateway cluster">
        <Button className="panel-toolbar-button" icon="refresh" text="Refresh" />
        <Button className="panel-toolbar-button" icon="cog" text="Configure" />
      </PageToolbar>

      <div className="router-demo-grid router-demo-grid--summary">
        <MetricCard
          icon="cloud-server"
          label="WAN Status"
          value="Active"
          meta="wan0 primary, backup standby"
          status="online"
        />
        <MetricCard
          icon="exchange"
          label="Interfaces"
          value={`${onlineInterfaces}/${interfaces.length}`}
          meta={`${warningInterfaces} degraded tunnel`}
          status={warningInterfaces > 0 ? 'warning' : 'online'}
        />
        <MetricCard
          icon="people"
          label="Clients"
          value={`${totalClients}`}
          meta={`${wifiClients} wireless associations`}
          status="online"
        />
        <MetricCard
          icon="shield"
          label="Threat Blocks"
          value="13.6k"
          meta="last 24 hours"
          status="warning"
        />
      </div>

      <div className="router-demo-overview-grid">
        <Panel
          title="WAN Throughput"
          icon="timeline-bar-chart"
          meta={
            <div className="router-demo-legend">
              <span className="router-demo-legend-dot router-demo-legend-dot--download" />
              <span>Down</span>
              <span className="router-demo-legend-dot router-demo-legend-dot--upload" />
              <span>Up</span>
            </div>
          }
        >
          <ThroughputChart />
        </Panel>

        <Panel title="System Load" icon="pulse" meta={<Tag minimal>42d 18h uptime</Tag>}>
          <div className="router-demo-stack">
            <UtilizationMeter value={0.38} label="CPU" />
            <UtilizationMeter value={0.54} label="Memory" />
            <UtilizationMeter value={0.29} label="Session table" />
            <UtilizationMeter value={0.66} label="NAT translation pool" />
          </div>
        </Panel>
      </div>

      <div className="router-demo-overview-grid router-demo-overview-grid--bottom">
        <Panel
          title="Interface Health"
          icon="exchange"
          meta={
            <Tag minimal intent="success">
              {onlineInterfaces}/{interfaces.length} available
            </Tag>
          }
        >
          <div className="router-demo-interface-list">
            {interfaces.map((item) => (
              <InterfaceStatusLine key={item.id} item={item} />
            ))}
          </div>
        </Panel>

        <Panel title="Network Map" icon="layout-hierarchy">
          <div className="router-demo-topology">
            <div className="router-demo-node router-demo-node--wan">
              <Icon icon="cloud" size={18} />
              <strong>ISP</strong>
              <span className="mono-data">2.5G</span>
            </div>
            <div className="router-demo-link router-demo-link--active" />
            <div className="router-demo-node router-demo-node--router">
              <Icon icon="server" size={18} />
              <strong>ER-04</strong>
              <span className="mono-data">10.42.0.1</span>
            </div>
            <div className="router-demo-link router-demo-link--split" />
            <div className="router-demo-node-stack">
              <div className="router-demo-node">
                <Icon icon="exchange" size={16} />
                <strong>LAN</strong>
                <span>{countStatus(interfaces, 'online')} links</span>
              </div>
              <div className="router-demo-node router-demo-node--warning">
                <Icon icon="route" size={16} />
                <strong>VPN</strong>
                <span>degraded</span>
              </div>
              <div className="router-demo-node">
                <Icon icon="antenna" size={16} />
                <strong>Wi-Fi</strong>
                <span>{wifiClients} clients</span>
              </div>
            </div>
          </div>
        </Panel>
      </div>
    </div>
  );
}

function Interfaces() {
  const [query, setQuery] = useState('');
  const [status, setStatus] = useState<'all' | RouterStatus>('all');

  const filtered = useMemo(
    () =>
      interfaces.filter((item) => {
        const normalizedQuery = query.toLowerCase();
        const matchesStatus = status === 'all' || item.status === status;
        const matchesQuery =
          item.name.toLowerCase().includes(normalizedQuery) ||
          item.address.toLowerCase().includes(normalizedQuery) ||
          item.type.toLowerCase().includes(normalizedQuery) ||
          item.mac.toLowerCase().includes(normalizedQuery);
        return matchesStatus && matchesQuery;
      }),
    [query, status],
  );

  return (
    <div className="router-demo-page">
      <PageToolbar title="Interfaces" subtitle={`${filtered.length} matching interfaces`}>
        <Button className="panel-toolbar-button" icon="add" text="Add interface" />
      </PageToolbar>

      <Card elevation={Elevation.ONE} className="router-demo-controls">
        <div className="router-demo-search">
          <SearchField value={query} onChange={setQuery} placeholder="Search interfaces..." />
        </div>
        <div className="filter-section">
          {(['all', 'online', 'warning', 'offline'] as const).map((item) => (
            <FilterPill
              key={item}
              value={item}
              label={item === 'all' ? 'All' : statusLabel(item)}
              active={status === item}
              status={item !== 'all' ? item : undefined}
              count={item === 'all' ? interfaces.length : countStatus(interfaces, item)}
              onSelect={setStatus}
            />
          ))}
        </div>
      </Card>

      <Panel title="Interface Table" icon="exchange">
        {filtered.length === 0 ? (
          <EmptyState
            icon="search"
            title="No interfaces found"
            description="Adjust the search term or status filter."
          />
        ) : (
          <div className="router-demo-table router-demo-table--interfaces">
            <div className="router-demo-table-header">
              <span>Status</span>
              <span>Name</span>
              <span>Type</span>
              <span>Address</span>
              <span>MAC</span>
              <span>RX</span>
              <span>TX</span>
              <span>Errors</span>
              <span>Uptime</span>
            </div>
            {filtered.map((item) => (
              <div key={item.id} className="router-demo-table-row">
                <StatusLed status={item.status} label={statusLabel(item.status)} />
                <strong>{item.name}</strong>
                <span>{item.type}</span>
                <span className="mono-data">{item.address}</span>
                <span className="mono-data">{item.mac}</span>
                <span className="mono-data">{formatMbps(item.rxMbps)}</span>
                <span className="mono-data">{formatMbps(item.txMbps)}</span>
                <span className="mono-data">{item.errors}</span>
                <span>{item.uptime}</span>
              </div>
            ))}
          </div>
        )}
      </Panel>
    </div>
  );
}

function Clients({
  clients,
  onToggleBlocked,
}: {
  clients: ConnectedClient[];
  onToggleBlocked: (id: string) => void;
}) {
  const [query, setQuery] = useState('');
  const [network, setNetwork] = useState('all');

  const networks = useMemo(
    () => ['all', ...Array.from(new Set(clients.map((client) => client.network)))],
    [clients],
  );

  const filtered = useMemo(
    () =>
      clients.filter((client) => {
        const normalizedQuery = query.toLowerCase();
        const matchesNetwork = network === 'all' || client.network === network;
        const matchesQuery =
          client.hostname.toLowerCase().includes(normalizedQuery) ||
          client.ip.includes(normalizedQuery) ||
          client.mac.toLowerCase().includes(normalizedQuery);
        return matchesNetwork && matchesQuery;
      }),
    [clients, network, query],
  );

  return (
    <div className="router-demo-page">
      <PageToolbar title="Clients" subtitle={`${filtered.length} devices on managed networks`}>
        <Button className="panel-toolbar-button" icon="blocked-person" text="Block selected" />
        <Button className="panel-toolbar-button" icon="export" text="Export leases" />
      </PageToolbar>

      <Card elevation={Elevation.ONE} className="router-demo-controls">
        <div className="router-demo-search">
          <SearchField value={query} onChange={setQuery} placeholder="Search clients..." />
        </div>
        <div className="filter-section">
          {networks.map((item) => (
            <FilterPill
              key={item}
              value={item}
              label={item === 'all' ? 'All' : item}
              active={network === item}
              count={
                item === 'all'
                  ? clients.length
                  : clients.filter((client) => client.network === item).length
              }
              onSelect={setNetwork}
            />
          ))}
        </div>
      </Card>

      <Panel title="Connected Devices" icon="people">
        <div className="router-demo-card-list">
          {filtered.map((client) => (
            <div key={client.id} className="router-demo-client-card">
              <div className="router-demo-client-main">
                <StatusLed status={client.blocked ? 'offline' : client.status} />
                <div>
                  <strong>{client.hostname}</strong>
                  <span className="mono-data">{client.ip}</span>
                </div>
              </div>
              <div className="router-demo-client-meta">
                <span>{client.network}</span>
                <span className="mono-data">{client.mac}</span>
                <span>{client.traffic}</span>
                <span>{client.lastSeen}</span>
              </div>
              <div className="router-demo-signal">
                <span>Signal</span>
                <ProgressBar
                  value={client.signal / 100}
                  intent={client.signal < 45 ? Intent.WARNING : Intent.SUCCESS}
                />
              </div>
              <Button
                className="panel-toolbar-button panel-toolbar-button--wide"
                icon={client.blocked ? 'unlock' : 'lock'}
                text={client.blocked ? 'Unblock' : 'Block'}
                intent={client.blocked ? Intent.SUCCESS : Intent.DANGER}
                onClick={() => onToggleBlocked(client.id)}
              />
            </div>
          ))}
        </div>
      </Panel>
    </div>
  );
}

function Wireless({
  networks,
  onToggleEnabled,
  onToggleHidden,
}: {
  networks: WifiNetwork[];
  onToggleEnabled: (id: string) => void;
  onToggleHidden: (id: string) => void;
}) {
  return (
    <div className="router-demo-page">
      <PageToolbar title="Wireless" subtitle="SSID, radio, and channel management">
        <Button className="panel-toolbar-button" icon="add" text="Add SSID" />
        <Button className="panel-toolbar-button" icon="refresh" text="Optimize channels" />
      </PageToolbar>

      <div className="router-demo-grid router-demo-grid--wireless">
        {networks.map((network) => (
          <Card key={network.id} elevation={Elevation.ONE} className="router-demo-network-card">
            <div className="router-demo-network-header">
              <div>
                <span className="router-demo-label">{network.band}</span>
                <h3>{network.ssid}</h3>
              </div>
              <StatusLed status={network.enabled ? 'online' : 'offline'} />
            </div>
            <div className="router-demo-network-facts">
              <span>Channel</span>
              <strong>{network.channel}</strong>
              <span>Security</span>
              <strong>{network.security}</strong>
              <span>Clients</span>
              <strong>{network.clients}</strong>
            </div>
            <UtilizationMeter value={network.utilization} label="Channel utilization" />
            <div className="router-demo-switch-row">
              <Switch
                checked={network.enabled}
                label="Enabled"
                onChange={() => onToggleEnabled(network.id)}
              />
              <Switch
                checked={network.hidden}
                label="Hidden SSID"
                onChange={() => onToggleHidden(network.id)}
              />
            </div>
          </Card>
        ))}
      </div>
    </div>
  );
}

function RoutesPage() {
  return (
    <div className="router-demo-page">
      <PageToolbar title="Routes" subtitle={`${routeEntries.length} active routing entries`}>
        <Button className="panel-toolbar-button" icon="add" text="Add route" />
        <Button className="panel-toolbar-button" icon="comparison" text="Validate" />
      </PageToolbar>

      <Panel title="Routing Table" icon="route">
        <div className="router-demo-table router-demo-table--routes">
          <div className="router-demo-table-header">
            <span>Status</span>
            <span>Destination</span>
            <span>Gateway</span>
            <span>Interface</span>
            <span>Metric</span>
            <span>Type</span>
          </div>
          {routeEntries.map((route) => (
            <div key={route.id} className="router-demo-table-row">
              <StatusLed status={route.status} label={statusLabel(route.status)} />
              <span className="mono-data">{route.destination}</span>
              <span className="mono-data">{route.gateway}</span>
              <strong>{route.iface}</strong>
              <span className="mono-data">{route.metric}</span>
              <Tag minimal>{route.type}</Tag>
            </div>
          ))}
        </div>
      </Panel>
    </div>
  );
}

function Firewall({
  rules,
  onToggleRule,
}: {
  rules: FirewallRule[];
  onToggleRule: (id: string) => void;
}) {
  const [action, setAction] = useState<'all' | FirewallAction>('all');
  const filtered = action === 'all' ? rules : rules.filter((rule) => rule.action === action);

  return (
    <div className="router-demo-page">
      <PageToolbar title="Firewall" subtitle={`${filtered.length} policy rules visible`}>
        <Button className="panel-toolbar-button" icon="add" text="Add rule" />
        <Button className="panel-toolbar-button" icon="shield" text="Threat feed" />
      </PageToolbar>

      <Card elevation={Elevation.ONE} className="router-demo-controls">
        <div className="filter-section">
          {(['all', 'allow', 'deny'] as const).map((item) => (
            <FilterPill
              key={item}
              value={item}
              label={item === 'all' ? 'All' : item.charAt(0).toUpperCase() + item.slice(1)}
              active={action === item}
              icon={
                item === 'deny' ? 'blocked-person' : item === 'allow' ? 'unlock' : 'filter-list'
              }
              count={
                item === 'all' ? rules.length : rules.filter((rule) => rule.action === item).length
              }
              onSelect={setAction}
            />
          ))}
        </div>
      </Card>

      <Panel title="Policy Rules" icon="shield">
        <div className="router-demo-table router-demo-table--firewall">
          <div className="router-demo-table-header">
            <span>Enabled</span>
            <span>Name</span>
            <span>Source</span>
            <span>Destination</span>
            <span>Service</span>
            <span>Action</span>
            <span>Hits</span>
          </div>
          {filtered.map((rule) => (
            <div key={rule.id} className="router-demo-table-row">
              <Switch checked={rule.enabled} onChange={() => onToggleRule(rule.id)} />
              <strong>{rule.name}</strong>
              <span>{rule.source}</span>
              <span>{rule.destination}</span>
              <span>{rule.service}</span>
              <Tag minimal intent={rule.action === 'allow' ? Intent.SUCCESS : Intent.DANGER}>
                {rule.action.toUpperCase()}
              </Tag>
              <span className="mono-data">{rule.hits.toLocaleString()}</span>
            </div>
          ))}
        </div>
      </Panel>
    </div>
  );
}

function LogsPage() {
  const [level, setLevel] = useState<'all' | LogLevel>('all');
  const [source, setSource] = useState('all');
  const sources = ['all', ...Array.from(new Set(logs.map((entry) => entry.source)))];
  const filtered = logs.filter((entry) => {
    const matchesLevel = level === 'all' || entry.level === level;
    const matchesSource = source === 'all' || entry.source === source;
    return matchesLevel && matchesSource;
  });

  return (
    <div className="router-demo-page">
      <PageToolbar title="Logs" subtitle={`${filtered.length} events in the live buffer`}>
        <Button className="panel-toolbar-button" icon="download" text="Download" />
        <Button className="panel-toolbar-button" icon="clean" text="Clear" />
      </PageToolbar>

      <Card elevation={Elevation.ONE} className="router-demo-controls">
        <div className="filter-section">
          {(['all', 'info', 'warning', 'critical'] as const).map((item) => (
            <FilterPill
              key={item}
              value={item}
              label={item === 'all' ? 'All' : item.charAt(0).toUpperCase() + item.slice(1)}
              active={level === item}
              count={
                item === 'all' ? logs.length : logs.filter((entry) => entry.level === item).length
              }
              onSelect={setLevel}
            />
          ))}
        </div>
        <HTMLSelect value={source} onChange={(event) => setSource(event.target.value)}>
          {sources.map((item) => (
            <option key={item} value={item}>
              {item === 'all' ? 'All sources' : item}
            </option>
          ))}
        </HTMLSelect>
      </Card>

      <Panel title="Event Stream" icon="manual">
        <div className="router-demo-log-list">
          {filtered.map((entry) => (
            <div key={entry.id} className="router-demo-log-row">
              <span className="mono-data">{entry.time}</span>
              <LogLevelTag level={entry.level} />
              <span>{entry.source}</span>
              <p>{entry.message}</p>
            </div>
          ))}
        </div>
      </Panel>
    </div>
  );
}

function Maintenance() {
  const [automaticUpdates, setAutomaticUpdates] = useState(true);
  const [diagnosticsEnabled, setDiagnosticsEnabled] = useState(true);

  return (
    <div className="router-demo-page">
      <PageToolbar title="Maintenance" subtitle="Firmware, backups, and diagnostics">
        <Button className="panel-toolbar-button" icon="cloud-upload" text="Upload firmware" />
        <Button
          className="panel-toolbar-button"
          icon="reset"
          text="Reboot"
          intent={Intent.DANGER}
        />
      </PageToolbar>

      <div className="router-demo-maintenance-grid">
        <Panel title="Firmware" icon="server-install">
          <div className="router-demo-maintenance-card">
            <div>
              <span className="router-demo-label">Current Version</span>
              <strong>EdgeOS 4.8.12</strong>
              <p>Build 2026.04.30-1742, signed release channel</p>
            </div>
            <Tag minimal intent="success">
              Current
            </Tag>
          </div>
          <div className="router-demo-switch-stack">
            <Switch
              checked={automaticUpdates}
              label="Automatic security updates"
              onChange={() => setAutomaticUpdates((value) => !value)}
            />
            <Switch
              checked={diagnosticsEnabled}
              label="Upload diagnostic bundles"
              onChange={() => setDiagnosticsEnabled((value) => !value)}
            />
          </div>
        </Panel>

        <Panel title="Backups" icon="database">
          <div className="router-demo-backup-list">
            {[
              ['21:00', 'Automatic snapshot', '128 KB'],
              ['18:00', 'Pre-policy-change', '126 KB'],
              ['Yesterday', 'Nightly retention', '127 KB'],
            ].map(([time, name, size]) => (
              <div key={`${time}-${name}`} className="router-demo-backup-row">
                <Icon icon="document" size={14} />
                <strong>{name}</strong>
                <span>{time}</span>
                <span className="mono-data">{size}</span>
              </div>
            ))}
          </div>
        </Panel>
      </div>
    </div>
  );
}

function RouterAdminDemo() {
  const navigate = useNavigate();
  const location = useLocation();
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [commandOpen, setCommandOpen] = useState(false);
  const [clients, setClients] = useState(initialClients);
  const [wifiNetworks, setWifiNetworks] = useState(initialWifiNetworks);
  const [firewallRules, setFirewallRules] = useState(initialFirewallRules);
  const navItems = flattenNavGroups(navGroups);
  const activeNavItem = navItems.find((item) =>
    item.href === '/' ? location.pathname === '/' : location.pathname.startsWith(item.href),
  );

  const toggleBlockedClient = (id: string) => {
    setClients((current) =>
      current.map((client) =>
        client.id === id
          ? { ...client, blocked: !client.blocked, status: client.blocked ? 'online' : 'offline' }
          : client,
      ),
    );
  };

  const toggleWifiEnabled = (id: string) => {
    setWifiNetworks((current) =>
      current.map((network) =>
        network.id === id ? { ...network, enabled: !network.enabled } : network,
      ),
    );
  };

  const toggleWifiHidden = (id: string) => {
    setWifiNetworks((current) =>
      current.map((network) =>
        network.id === id ? { ...network, hidden: !network.hidden } : network,
      ),
    );
  };

  const toggleFirewallRule = (id: string) => {
    setFirewallRules((current) =>
      current.map((rule) => (rule.id === id ? { ...rule, enabled: !rule.enabled } : rule)),
    );
  };

  const commandPalette = (
    <CommandPaletteShell
      open={commandOpen}
      onOpenChange={setCommandOpen}
      onToggle={() => setCommandOpen((open) => !open)}
      placeholder="Search router admin..."
    >
      <Command.Group heading="Pages">
        {navItems.map((item) => (
          <Command.Item
            key={item.href}
            value={item.label}
            onSelect={() => {
              setCommandOpen(false);
              navigate(item.href);
            }}
          >
            <Icon icon={item.icon} size={16} />
            <span className="cmdk-item-label">{item.label}</span>
          </Command.Item>
        ))}
      </Command.Group>
      <Command.Group heading="Interfaces">
        {interfaces.map((item) => (
          <Command.Item
            key={item.id}
            value={item.name}
            keywords={[item.address, item.type]}
            onSelect={() => {
              setCommandOpen(false);
              navigate('/interfaces');
            }}
          >
            <StatusLed status={item.status} className="cmdk-status-led" />
            <span className="cmdk-item-label">{item.name}</span>
            <span className="cmdk-item-meta">{item.address}</span>
          </Command.Item>
        ))}
      </Command.Group>
      <Command.Group heading="Clients">
        {clients.map((client) => (
          <Command.Item
            key={client.id}
            value={client.hostname}
            keywords={[client.ip, client.mac, client.network]}
            onSelect={() => {
              setCommandOpen(false);
              navigate('/clients');
            }}
          >
            <StatusLed
              status={client.blocked ? 'offline' : client.status}
              className="cmdk-status-led"
            />
            <span className="cmdk-item-label">{client.hostname}</span>
            <span className="cmdk-item-meta">{client.ip}</span>
          </Command.Item>
        ))}
      </Command.Group>
    </CommandPaletteShell>
  );

  return (
    <AppShell
      productName="Router Admin"
      collapsedProductName="RA"
      navGroups={navGroups}
      projects={projects}
      user={user}
      version="demo"
      sidebarCollapsed={sidebarCollapsed}
      onToggleSidebar={() => setSidebarCollapsed((collapsed) => !collapsed)}
      onOpenCommandPalette={() => setCommandOpen(true)}
      onLogout={() => undefined}
      breadcrumb={activeNavItem?.label ?? 'Router Admin'}
      navBadges={{
        Interfaces: { count: interfaces.length },
        Clients: { count: clients.length },
        Wireless: {
          status:
            countStatus(
              wifiNetworks.map((network) => ({ status: network.enabled ? 'online' : 'offline' })),
              'offline',
            ) > 0
              ? 'warning'
              : 'online',
        },
        Firewall: {
          status: firewallRules.some((rule) => rule.action === 'deny' && rule.enabled)
            ? 'online'
            : 'warning',
        },
        Routes: { status: 'warning' },
        Logs: { count: logs.filter((entry) => entry.level !== 'info').length },
      }}
      commandPalette={commandPalette}
    >
      <Routes>
        <Route path="/" element={<Overview clients={clients} wifiNetworks={wifiNetworks} />} />
        <Route path="/interfaces" element={<Interfaces />} />
        <Route
          path="/clients"
          element={<Clients clients={clients} onToggleBlocked={toggleBlockedClient} />}
        />
        <Route
          path="/wireless"
          element={
            <Wireless
              networks={wifiNetworks}
              onToggleEnabled={toggleWifiEnabled}
              onToggleHidden={toggleWifiHidden}
            />
          }
        />
        <Route path="/routes" element={<RoutesPage />} />
        <Route
          path="/firewall"
          element={<Firewall rules={firewallRules} onToggleRule={toggleFirewallRule} />}
        />
        <Route path="/logs" element={<LogsPage />} />
        <Route path="/maintenance" element={<Maintenance />} />
      </Routes>
    </AppShell>
  );
}

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <ThemeProvider>
      <HashRouter>
        <RouterAdminDemo />
      </HashRouter>
    </ThemeProvider>
  </React.StrictMode>,
);
