import React, { useMemo, useState } from 'react';
import ReactDOM from 'react-dom/client';
import { HashRouter, Route, Routes, useNavigate } from 'react-router-dom';
import { Button, Card, Elevation, Icon, Tag } from '@blueprintjs/core';
import { Command } from 'cmdk';
import type { NavGroup, Project, User } from '@extrittio/navigation';
import { AppShell } from '@extrittio/app-shell';
import { CommandPaletteShell } from '@extrittio/command-palette';
import { createApiClient } from '@extrittio/data-client';
import { ThemeProvider } from '@extrittio/theme';
import { EmptyState, FilterPill, MainToolbar, SearchField, StatusLed } from '@extrittio/ui';

import '@blueprintjs/core/lib/css/blueprint.css';
import '@blueprintjs/icons/lib/css/blueprint-icons.css';
import '@extrittio/theme/theme.css';
import './router-admin-demo.css';

type InterfaceStatus = 'online' | 'warning' | 'offline';

interface RouterInterface {
  id: string;
  name: string;
  status: InterfaceStatus;
  type: 'WAN' | 'LAN' | 'VPN';
  address: string;
  throughput: string;
  errors: number;
}

const navGroups: NavGroup[] = [
  {
    label: 'Router',
    items: [
      { label: 'Overview', icon: 'dashboard', href: '/' },
      { label: 'Interfaces', icon: 'exchange', href: '/interfaces' },
      { label: 'Routes', icon: 'route', href: '/routes' },
      { label: 'Logs', icon: 'manual', href: '/logs' },
    ],
  },
  {
    label: 'Operations',
    items: [
      { label: 'Firewall', icon: 'shield', href: '/firewall' },
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
    throughput: '814 Mbps',
    errors: 0,
  },
  {
    id: 'lan0',
    name: 'lan0',
    status: 'online',
    type: 'LAN',
    address: '10.42.0.1/24',
    throughput: '2.1 Gbps',
    errors: 0,
  },
  {
    id: 'vpn0',
    name: 'vpn0',
    status: 'warning',
    type: 'VPN',
    address: '172.18.6.1/24',
    throughput: '164 Mbps',
    errors: 12,
  },
  {
    id: 'backup-wan',
    name: 'backup-wan',
    status: 'offline',
    type: 'WAN',
    address: '203.0.113.9/30',
    throughput: '0 Mbps',
    errors: 0,
  },
];

const apiClient = createApiClient({
  baseUrl: '/router-admin-demo/api',
  getToken: () => 'demo-token',
});

void apiClient;

function statusCount(status: InterfaceStatus) {
  return interfaces.filter((item) => item.status === status).length;
}

function Overview() {
  const online = statusCount('online');
  const warning = statusCount('warning');
  const offline = statusCount('offline');

  return (
    <div className="router-demo-page">
      <MainToolbar ariaLabel="Router overview toolbar">
        <div className="router-demo-toolbar">
          <div>
            <h2>Edge Router</h2>
            <p>Site 04 gateway cluster</p>
          </div>
          <div className="router-demo-actions">
            <Button className="panel-toolbar-button" icon="refresh" text="Refresh" />
            <Button className="panel-toolbar-button" icon="cog" text="Configure" />
          </div>
        </div>
      </MainToolbar>

      <div className="router-demo-grid router-demo-grid--summary">
        <Card elevation={Elevation.ONE} className="router-demo-card">
          <span className="router-demo-label">Interfaces</span>
          <strong>{interfaces.length}</strong>
          <span className="router-demo-muted">{online} online</span>
        </Card>
        <Card elevation={Elevation.ONE} className="router-demo-card">
          <span className="router-demo-label">Warnings</span>
          <strong>{warning}</strong>
          <span className="router-demo-muted">VPN degradation</span>
        </Card>
        <Card elevation={Elevation.ONE} className="router-demo-card">
          <span className="router-demo-label">Offline</span>
          <strong>{offline}</strong>
          <span className="router-demo-muted">Backup WAN disabled</span>
        </Card>
      </div>

      <Card elevation={Elevation.ONE} className="router-demo-panel">
        <div className="router-demo-panel-header">
          <h3>Interface Health</h3>
          <Tag minimal intent="success">
            {online}/{interfaces.length} available
          </Tag>
        </div>
        <div className="router-demo-interface-list">
          {interfaces.map((item) => (
            <div key={item.id} className="router-demo-interface-row">
              <StatusLed status={item.status} />
              <strong>{item.name}</strong>
              <span>{item.type}</span>
              <span className="mono-data">{item.address}</span>
              <span className="mono-data">{item.throughput}</span>
            </div>
          ))}
        </div>
      </Card>
    </div>
  );
}

function Interfaces() {
  const [query, setQuery] = useState('');
  const [status, setStatus] = useState<'all' | InterfaceStatus>('all');

  const filtered = useMemo(
    () =>
      interfaces.filter((item) => {
        const matchesStatus = status === 'all' || item.status === status;
        const matchesQuery =
          item.name.toLowerCase().includes(query.toLowerCase()) ||
          item.address.toLowerCase().includes(query.toLowerCase()) ||
          item.type.toLowerCase().includes(query.toLowerCase());
        return matchesStatus && matchesQuery;
      }),
    [query, status],
  );

  return (
    <div className="router-demo-page">
      <MainToolbar ariaLabel="Interface toolbar">
        <div className="router-demo-toolbar">
          <div>
            <h2>Interfaces</h2>
            <p>{filtered.length} matching interfaces</p>
          </div>
          <Button className="panel-toolbar-button" icon="add" text="Add interface" />
        </div>
      </MainToolbar>

      <Card elevation={Elevation.ONE} className="router-demo-controls">
        <div className="router-demo-search">
          <SearchField value={query} onChange={setQuery} placeholder="Search interfaces..." />
        </div>
        <div className="filter-section">
          {(['all', 'online', 'warning', 'offline'] as const).map((item) => (
            <FilterPill
              key={item}
              value={item}
              label={item === 'all' ? 'All' : item.charAt(0).toUpperCase() + item.slice(1)}
              active={status === item}
              status={item !== 'all' ? item : undefined}
              count={item === 'all' ? interfaces.length : statusCount(item)}
              onSelect={setStatus}
            />
          ))}
        </div>
      </Card>

      <Card elevation={Elevation.ONE} className="router-demo-panel">
        {filtered.length === 0 ? (
          <EmptyState
            icon="search"
            title="No interfaces found"
            description="Adjust the search term or status filter."
          />
        ) : (
          <div className="router-demo-table">
            <div className="router-demo-table-header">
              <span>Status</span>
              <span>Name</span>
              <span>Type</span>
              <span>Address</span>
              <span>Throughput</span>
              <span>Errors</span>
            </div>
            {filtered.map((item) => (
              <div key={item.id} className="router-demo-table-row">
                <StatusLed status={item.status} />
                <strong>{item.name}</strong>
                <span>{item.type}</span>
                <span className="mono-data">{item.address}</span>
                <span className="mono-data">{item.throughput}</span>
                <span className="mono-data">{item.errors}</span>
              </div>
            ))}
          </div>
        )}
      </Card>
    </div>
  );
}

function PlaceholderPage({
  title,
  icon,
}: {
  title: string;
  icon: 'route' | 'manual' | 'shield' | 'wrench';
}) {
  return (
    <div className="router-demo-page">
      <EmptyState
        icon={icon}
        title={title}
        description="This route is provided by the demo app, not the Extrittio domain."
      />
    </div>
  );
}

function RouterAdminDemo() {
  const navigate = useNavigate();
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [commandOpen, setCommandOpen] = useState(false);

  const commandPalette = (
    <CommandPaletteShell
      open={commandOpen}
      onOpenChange={setCommandOpen}
      onToggle={() => setCommandOpen((open) => !open)}
      placeholder="Search router admin..."
    >
      <Command.Group heading="Pages">
        {navGroups.flatMap((group) =>
          group.items.map((item) => (
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
          )),
        )}
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
      breadcrumb="Router Admin"
      navBadges={{
        Interfaces: { count: interfaces.length },
        Routes: { status: 'warning' },
      }}
      commandPalette={commandPalette}
    >
      <Routes>
        <Route path="/" element={<Overview />} />
        <Route path="/interfaces" element={<Interfaces />} />
        <Route path="/routes" element={<PlaceholderPage title="Routes" icon="route" />} />
        <Route path="/logs" element={<PlaceholderPage title="Logs" icon="manual" />} />
        <Route path="/firewall" element={<PlaceholderPage title="Firewall" icon="shield" />} />
        <Route
          path="/maintenance"
          element={<PlaceholderPage title="Maintenance" icon="wrench" />}
        />
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
