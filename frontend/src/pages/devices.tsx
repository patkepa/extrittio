import { useState } from 'react';
import {
  Card,
  Elevation,
  H3,
  HTMLTable,
  Tag,
  Button,
  InputGroup,
  Icon,
  Drawer,
  Position,
  H4,
  Callout,
  Divider,
  Tabs,
  Tab,
} from '@blueprintjs/core';
import './devices.css';

// Pure SVG sparkline (same pattern as dashboard)
const Sparkline = ({ data, color, width = 80, height = 24 }: { data: number[]; color: string; width?: number; height?: number }) => {
  const min = Math.min(...data);
  const max = Math.max(...data);
  const range = max - min || 1;
  const points = data.map((v, i) => {
    const x = (i / (data.length - 1)) * width;
    const y = height - ((v - min) / range) * (height - 4) - 2;
    return `${x},${y}`;
  }).join(' ');
  const areaPoints = `0,${height} ${points} ${width},${height}`;

  return (
    <svg width={width} height={height} viewBox={`0 0 ${width} ${height}`} style={{ display: 'block' }}>
      <defs>
        <linearGradient id={`dsp-${color.replace(/[^a-zA-Z0-9]/g, '')}`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor={color} stopOpacity={0.3} />
          <stop offset="100%" stopColor={color} stopOpacity={0} />
        </linearGradient>
      </defs>
      <polygon points={areaPoints} fill={`url(#dsp-${color.replace(/[^a-zA-Z0-9]/g, '')})`} />
      <polyline points={points} fill="none" stroke={color} strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
};

interface Device {
  id: string;
  name: string;
  type: string;
  status: 'online' | 'offline' | 'warning';
  lastSeen: string;
  firmware: string;
  location: string;
  uptime: string;
  activity: number[];
}

const devices: Device[] = [
  { id: 'DEV-001', name: 'Temperature Sensor 01', type: 'Sensor', status: 'online', lastSeen: '2 min ago', firmware: 'v2.1.3', location: 'Building A — Floor 2', uptime: '45d 12h', activity: [20, 35, 28, 45, 38, 52, 44] },
  { id: 'DEV-002', name: 'Smart Camera 03', type: 'Camera', status: 'online', lastSeen: '5 min ago', firmware: 'v1.8.2', location: 'Entrance — Main Gate', uptime: '12d 8h', activity: [50, 42, 55, 48, 60, 52, 58] },
  { id: 'DEV-003', name: 'Motion Detector 12', type: 'Sensor', status: 'offline', lastSeen: '2h ago', firmware: 'v2.0.1', location: 'Warehouse — Zone C', uptime: '0d', activity: [30, 25, 20, 15, 10, 5, 0] },
  { id: 'DEV-004', name: 'Humidity Sensor 05', type: 'Sensor', status: 'warning', lastSeen: '1 min ago', firmware: 'v2.1.1', location: 'Server Room', uptime: '89d 4h', activity: [40, 45, 60, 75, 80, 85, 90] },
  { id: 'DEV-005', name: 'Smart Lock 08', type: 'Actuator', status: 'online', lastSeen: '30s ago', firmware: 'v3.0.0', location: 'Office — Room 204', uptime: '156d 2h', activity: [10, 15, 12, 18, 14, 20, 16] },
];

type SortField = 'name' | 'status' | 'lastSeen' | 'uptime';
type SortDir = 'asc' | 'desc';

const telemetryData = {
  cpu: [32, 45, 38, 42, 55, 48, 52, 44, 40, 38, 42, 50],
  memory: [60, 62, 58, 65, 63, 67, 64, 68, 62, 60, 65, 63],
  signal: [85, 82, 88, 84, 90, 86, 88, 85, 83, 87, 89, 85],
};

const logEntries = [
  { time: '14:32:01', level: 'INFO', message: 'Device heartbeat received' },
  { time: '14:30:45', level: 'INFO', message: 'Telemetry data uploaded (128 bytes)' },
  { time: '14:28:12', level: 'WARN', message: 'Signal strength below threshold' },
  { time: '14:25:00', level: 'INFO', message: 'Configuration sync completed' },
  { time: '14:20:33', level: 'ERROR', message: 'Connection timeout — retrying' },
  { time: '14:18:15', level: 'INFO', message: 'Firmware check: up to date' },
];

const configEntries = [
  { key: 'reporting_interval', value: '30s' },
  { key: 'max_retries', value: '3' },
  { key: 'protocol', value: 'MQTT v5' },
  { key: 'encryption', value: 'TLS 1.3' },
  { key: 'data_format', value: 'Protobuf' },
  { key: 'keepalive', value: '60s' },
];

export const Devices = () => {
  const [searchQuery, setSearchQuery] = useState('');
  const [filterStatus, setFilterStatus] = useState<string>('all');
  const [selectedDevice, setSelectedDevice] = useState<Device | null>(null);
  const [isDrawerOpen, setIsDrawerOpen] = useState(false);
  const [sortField, setSortField] = useState<SortField>('name');
  const [sortDir, setSortDir] = useState<SortDir>('asc');
  const [drawerTab, setDrawerTab] = useState('overview');

  const filteredDevices = devices
    .filter((device) => {
      const matchesSearch =
        device.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
        device.type.toLowerCase().includes(searchQuery.toLowerCase()) ||
        device.location.toLowerCase().includes(searchQuery.toLowerCase());
      const matchesStatus = filterStatus === 'all' || device.status === filterStatus;
      return matchesSearch && matchesStatus;
    })
    .sort((a, b) => {
      const dir = sortDir === 'asc' ? 1 : -1;
      if (sortField === 'name') return a.name.localeCompare(b.name) * dir;
      if (sortField === 'status') return a.status.localeCompare(b.status) * dir;
      return 0;
    });

  const handleSort = (field: SortField) => {
    if (sortField === field) {
      setSortDir(sortDir === 'asc' ? 'desc' : 'asc');
    } else {
      setSortField(field);
      setSortDir('asc');
    }
  };

  const handleViewDevice = (device: Device) => {
    setSelectedDevice(device);
    setDrawerTab('overview');
    setIsDrawerOpen(true);
  };

  const statusCounts = {
    all: devices.length,
    online: devices.filter((d) => d.status === 'online').length,
    offline: devices.filter((d) => d.status === 'offline').length,
    warning: devices.filter((d) => d.status === 'warning').length,
  };

  const SortHeader = ({ field, children }: { field: SortField; children: React.ReactNode }) => (
    <th className="sortable-th" onClick={() => handleSort(field)}>
      <span className="th-content">
        {children}
        {sortField === field && (
          <Icon icon={sortDir === 'asc' ? 'chevron-up' : 'chevron-down'} size={12} />
        )}
      </span>
    </th>
  );

  const getStatusColor = (status: string) => {
    switch (status) {
      case 'online': return '#0F9960';
      case 'offline': return '#E76A6E';
      case 'warning': return '#D99E0B';
      default: return '#888';
    }
  };

  return (
    <div className="devices-page">
      {/* Header */}
      <div className="page-header">
        <div>
          <H3>Devices</H3>
          <p className="page-description">
            {filteredDevices.length} of {devices.length} devices
          </p>
        </div>
        <Button intent="primary" icon="add">
          Add Device
        </Button>
      </div>

      {/* Filters and Search */}
      <Card elevation={Elevation.ONE} className="devices-controls">
        <div className="controls-row">
          <div className="search-section">
            <InputGroup
              leftIcon="search"
              placeholder="Search by name, type, or location..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              fill
              rightElement={
                searchQuery ? (
                  <Button icon="cross" minimal onClick={() => setSearchQuery('')} />
                ) : undefined
              }
            />
          </div>

          <div className="filter-section">
            {(['all', 'online', 'offline', 'warning'] as const).map((status) => (
              <button
                key={status}
                className={`filter-pill ${filterStatus === status ? 'active' : ''} ${status !== 'all' ? `pill-${status}` : ''}`}
                onClick={() => setFilterStatus(status)}
              >
                {status !== 'all' && <span className={`status-led status-led--${status}`} />}
                <span className="pill-label">{status === 'all' ? 'All' : status.charAt(0).toUpperCase() + status.slice(1)}</span>
                <span className="pill-count mono-data">{statusCounts[status]}</span>
              </button>
            ))}
          </div>
        </div>
      </Card>

      {/* Devices Table */}
      <Card elevation={Elevation.TWO} className="devices-card">
        {filteredDevices.length === 0 ? (
          <div className="empty-state">
            <Icon icon="search" size={48} />
            <H4>No devices found</H4>
            <p>Try adjusting your search or filter criteria</p>
          </div>
        ) : (
          <HTMLTable interactive className="devices-table">
            <thead>
              <tr>
                <th style={{ width: 40 }}></th>
                <SortHeader field="name">Name</SortHeader>
                <th>Type</th>
                <th>Location</th>
                <th>Last Seen</th>
                <th>Firmware</th>
                <th style={{ width: 80 }}>Activity</th>
                <th>Uptime</th>
                <th className="actions-column">Actions</th>
              </tr>
            </thead>
            <tbody>
              {filteredDevices.map((device, idx) => (
                <tr
                  key={device.id}
                  className={`device-row ${selectedDevice?.id === device.id ? 'row-selected' : ''}`}
                  onClick={() => handleViewDevice(device)}
                  style={{ animationDelay: `${idx * 30}ms` }}
                >
                  <td>
                    <span className={`status-led status-led--${device.status}`} />
                  </td>
                  <td>
                    <div className="device-name-cell">
                      <strong>{device.name}</strong>
                      <span className="device-id mono-data">{device.id}</span>
                    </div>
                  </td>
                  <td>
                    <Tag minimal>{device.type}</Tag>
                  </td>
                  <td className="location-cell">{device.location}</td>
                  <td>
                    <span className="mono-data">{device.lastSeen}</span>
                  </td>
                  <td>
                    <code className="firmware-badge">{device.firmware}</code>
                  </td>
                  <td>
                    <div className="row-sparkline">
                      <Sparkline data={device.activity} color={getStatusColor(device.status)} />
                    </div>
                  </td>
                  <td>
                    <span className="mono-data">{device.uptime}</span>
                  </td>
                  <td className="actions-column" onClick={(e) => e.stopPropagation()}>
                    <Button icon="eye-open" minimal small onClick={() => handleViewDevice(device)} title="View Details" />
                    <Button icon="edit" minimal small title="Edit Device" />
                    <Button icon="trash" minimal small intent="danger" title="Delete Device" />
                  </td>
                </tr>
              ))}
            </tbody>
          </HTMLTable>
        )}
      </Card>

      {/* Device Detail Drawer */}
      <Drawer
        icon="info-sign"
        title="Device Details"
        isOpen={isDrawerOpen}
        onClose={() => setIsDrawerOpen(false)}
        position={Position.RIGHT}
        size="520px"
      >
        <div className="drawer-content">
          {selectedDevice && (
            <>
              <div className="drawer-header">
                <div className="device-status-banner">
                  <span className={`status-led status-led--${selectedDevice.status}`} style={{ width: 10, height: 10 }} />
                  <div>
                    <H4 style={{ margin: 0 }}>{selectedDevice.name}</H4>
                    <p style={{ margin: 0 }} className="banner-subtitle">
                      <span className="mono-data">{selectedDevice.id}</span>
                      <span className="banner-sep">|</span>
                      {selectedDevice.type}
                      <span className="banner-sep">|</span>
                      <span style={{ textTransform: 'uppercase', fontWeight: 700, fontSize: 12, letterSpacing: '0.06em' }}>
                        {selectedDevice.status}
                      </span>
                    </p>
                  </div>
                </div>
              </div>

              <div className="drawer-tabs">
                <Tabs
                  id="device-tabs"
                  selectedTabId={drawerTab}
                  onChange={(newTab) => setDrawerTab(newTab as string)}
                >
                  <Tab id="overview" title="Overview" />
                  <Tab id="telemetry" title="Telemetry" />
                  <Tab id="logs" title="Logs" />
                  <Tab id="config" title="Config" />
                </Tabs>
              </div>

              <div className="drawer-body">
                {drawerTab === 'overview' && (
                  <>
                    {selectedDevice.status === 'offline' && (
                      <Callout intent="danger" icon="error" style={{ marginBottom: 16 }}>
                        Device offline — last seen {selectedDevice.lastSeen}
                      </Callout>
                    )}
                    {selectedDevice.status === 'warning' && (
                      <Callout intent="warning" icon="warning-sign" style={{ marginBottom: 16 }}>
                        Device reporting warnings. Check telemetry.
                      </Callout>
                    )}

                    <div className="detail-grid">
                      <div className="detail-item">
                        <span className="section-label">Device ID</span>
                        <span className="detail-value mono-data">{selectedDevice.id}</span>
                      </div>
                      <div className="detail-item">
                        <span className="section-label">Location</span>
                        <span className="detail-value">{selectedDevice.location}</span>
                      </div>
                      <div className="detail-item">
                        <span className="section-label">Firmware</span>
                        <span className="detail-value mono-data">{selectedDevice.firmware}</span>
                      </div>
                      <div className="detail-item">
                        <span className="section-label">Last Seen</span>
                        <span className="detail-value mono-data">{selectedDevice.lastSeen}</span>
                      </div>
                      <div className="detail-item">
                        <span className="section-label">Uptime</span>
                        <span className="detail-value mono-data">{selectedDevice.uptime}</span>
                      </div>
                    </div>

                    <Divider style={{ margin: '16px 0' }} />

                    <span className="section-label">Quick Actions</span>
                    <div className="quick-actions">
                      <Button icon="refresh" fill>Restart</Button>
                      <Button icon="cloud-upload" fill>Update FW</Button>
                      <Button icon="chart" fill>Telemetry</Button>
                      <Button icon="cog" fill>Configure</Button>
                    </div>
                  </>
                )}

                {drawerTab === 'telemetry' && (
                  <div className="telemetry-tab">
                    {[
                      { label: 'CPU Usage', data: telemetryData.cpu, color: '#2965CC', unit: '%' },
                      { label: 'Memory', data: telemetryData.memory, color: '#0F9960', unit: '%' },
                      { label: 'Signal Strength', data: telemetryData.signal, color: '#D99E0B', unit: 'dBm' },
                    ].map((metric) => (
                      <div key={metric.label} className="telemetry-chart">
                        <div className="telemetry-header">
                          <span className="section-label">{metric.label}</span>
                          <span className="mono-data" style={{ fontSize: 14, color: metric.color }}>
                            {metric.data[metric.data.length - 1]}{metric.unit}
                          </span>
                        </div>
                        <Sparkline data={metric.data} color={metric.color} width={440} height={60} />
                      </div>
                    ))}
                  </div>
                )}

                {drawerTab === 'logs' && (
                  <div className="logs-tab">
                    {logEntries.map((entry, i) => (
                      <div key={i} className="log-entry">
                        <span className="log-time mono-data">{entry.time}</span>
                        <span className={`log-level log-level--${entry.level.toLowerCase()} mono-data`}>
                          {entry.level}
                        </span>
                        <span className="log-message">{entry.message}</span>
                      </div>
                    ))}
                  </div>
                )}

                {drawerTab === 'config' && (
                  <div className="config-tab">
                    {configEntries.map((entry) => (
                      <div key={entry.key} className="config-row">
                        <span className="config-key mono-data">{entry.key}</span>
                        <span className="config-value mono-data">{entry.value}</span>
                      </div>
                    ))}
                  </div>
                )}
              </div>

              <div className="drawer-footer">
                <Button onClick={() => setIsDrawerOpen(false)}>Close</Button>
                <Button intent="primary" icon="edit">Edit Device</Button>
              </div>
            </>
          )}
        </div>
      </Drawer>
    </div>
  );
};
