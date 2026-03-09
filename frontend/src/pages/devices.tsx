import { useState } from 'react';
import { useSearchParams } from 'react-router-dom';
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
  Spinner,
  Dialog,
  DialogBody,
  DialogFooter,
  FormGroup,
  HTMLSelect,
} from '@blueprintjs/core';
import { AreaChart, Area, ResponsiveContainer } from 'recharts';
import { useDevices, useCreateDevice, useDeleteDevice, useRestartDevice } from '../hooks/use-devices';
import { useDeviceTelemetry } from '../hooks/use-telemetry';
import { useDeviceTypes } from '../hooks/use-device-types';
import { useFleets } from '../hooks/use-fleets';
import { useUIStore } from '../stores/ui-store';
import type { Device } from '../types/api';
import './devices.css';

type SortField = 'name' | 'status' | 'last_seen' | 'uptime';
type SortDir = 'asc' | 'desc';

function generateSparkline(id: string): number[] {
  let hash = 0;
  for (const ch of id) hash = ((hash << 5) - hash + ch.charCodeAt(0)) | 0;
  return Array.from({ length: 7 }, (_, i) => Math.abs((hash * (i + 1)) % 100));
}

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
  const [searchParams, setSearchParams] = useSearchParams();
  const [searchQuery, setSearchQuery] = useState('');
  const [filterStatus, setFilterStatus] = useState<string>('all');
  const [selectedDevice, setSelectedDevice] = useState<Device | null>(null);
  const [isDrawerOpen, setIsDrawerOpen] = useState(false);
  const [sortField, setSortField] = useState<SortField>('name');
  const [sortDir, setSortDir] = useState<SortDir>('asc');
  const [drawerTab, setDrawerTab] = useState('overview');
  const { isAddDeviceDialogOpen: isAddDialogOpen, openAddDeviceDialog, closeAddDeviceDialog } = useUIStore();
  const [newDevice, setNewDevice] = useState({
    name: '',
    device_type_id: 0,
    fleet_id: undefined as number | undefined,
    location: '',
    firmware: '',
  });

  // Fleet filter from URL query param
  const filterFleetId = searchParams.get('fleet_id') ? Number(searchParams.get('fleet_id')) : null;

  const setFilterFleetId = (fleetId: number | null) => {
    if (fleetId === null) {
      searchParams.delete('fleet_id');
    } else {
      searchParams.set('fleet_id', String(fleetId));
    }
    setSearchParams(searchParams);
  };

  const { data: devices = [], isLoading, error } = useDevices(
    filterFleetId ? { fleet_id: filterFleetId } : undefined
  );
  const { data: deviceTypes = [] } = useDeviceTypes();
  const { data: fleets = [] } = useFleets();
  const createDeviceMutation = useCreateDevice();
  const deleteDeviceMutation = useDeleteDevice();
  const restartDeviceMutation = useRestartDevice();

  // Set default device_type_id when device types load
  const defaultTypeId = deviceTypes.find((dt) => dt.name === 'default')?.id ?? deviceTypes[0]?.id ?? 0;

  const handleAddDevice = () => {
    createDeviceMutation.mutate(
      {
        name: newDevice.name,
        device_type_id: newDevice.device_type_id || defaultTypeId,
        fleet_id: newDevice.fleet_id,
        location: newDevice.location || undefined,
        firmware: newDevice.firmware || undefined,
      },
      {
        onSuccess: () => {
          closeAddDeviceDialog();
          setNewDevice({ name: '', device_type_id: 0, fleet_id: undefined, location: '', firmware: '' });
        },
      }
    );
  };

  const { data: telemetryRecords = [] } = useDeviceTelemetry(
    selectedDevice?.id ?? null,
    { limit: 50 }
  );

  const telemetryChartData = telemetryRecords
    .slice()
    .reverse()
    .map((r) => ({
      temperature: r.temperature,
      humidity: r.humidity,
      battery: r.battery_level,
    }));

  const filteredDevices = devices
    .filter((device) => {
      const matchesSearch =
        device.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
        device.device_type_name.toLowerCase().includes(searchQuery.toLowerCase()) ||
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
      default: return '#888';
    }
  };

  // Get the fleet name for the active fleet filter
  const activeFleetName = filterFleetId ? fleets.find((f) => f.id === filterFleetId)?.name : null;

  if (error) {
    return (
      <div className="devices-page">
        <Callout intent="danger" icon="error">
          Failed to load devices. Is the backend running?
        </Callout>
      </div>
    );
  }

  if (isLoading) {
    return (
      <div className="devices-page">
        <Spinner />
      </div>
    );
  }

  return (
    <div className="devices-page">
      {/* Header */}
      <div className="page-header">
        <div>
          <H3>Devices</H3>
          <p className="page-description">
            {filteredDevices.length} of {devices.length} devices
            {activeFleetName && (
              <span> in <strong>{activeFleetName}</strong></span>
            )}
          </p>
        </div>
        <Button intent="primary" icon="add" onClick={() => setIsAddDialogOpen(true)}>
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
            {(['all', 'online', 'offline'] as const).map((status) => (
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

        {/* Fleet filter */}
        {fleets.length > 0 && (
          <div className="controls-row" style={{ marginTop: 10 }}>
            <div className="filter-section">
              <button
                className={`filter-pill ${filterFleetId === null ? 'active' : ''}`}
                onClick={() => setFilterFleetId(null)}
              >
                <span className="pill-label">All Fleets</span>
              </button>
              {fleets.map((fleet) => (
                <button
                  key={fleet.id}
                  className={`filter-pill ${filterFleetId === fleet.id ? 'active' : ''}`}
                  onClick={() => setFilterFleetId(fleet.id)}
                >
                  <span className="pill-label">{fleet.name}</span>
                  <span className="pill-count mono-data">{fleet.device_count}</span>
                </button>
              ))}
            </div>
          </div>
        )}
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
                <th>Fleet</th>
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
                    <Tag minimal>{device.device_type_name}</Tag>
                  </td>
                  <td>
                    {device.fleet_name ? (
                      <Tag minimal intent="primary">{device.fleet_name}</Tag>
                    ) : (
                      <span style={{ color: 'hsl(var(--muted))', fontSize: 12 }}>—</span>
                    )}
                  </td>
                  <td className="location-cell">{device.location}</td>
                  <td>
                    <span className="mono-data">{device.last_seen}</span>
                  </td>
                  <td>
                    <code className="firmware-badge">{device.firmware}</code>
                  </td>
                  <td>
                    <div className="row-sparkline">
                      <ResponsiveContainer width="100%" height={24}>
                        <AreaChart data={generateSparkline(device.id).map((v, i) => ({ v, i }))}>
                          <Area
                            type="monotone"
                            dataKey="v"
                            stroke={getStatusColor(device.status)}
                            strokeWidth={1}
                            fill={getStatusColor(device.status)}
                            fillOpacity={0.15}
                            dot={false}
                            isAnimationActive={false}
                          />
                        </AreaChart>
                      </ResponsiveContainer>
                    </div>
                  </td>
                  <td>
                    <span className="mono-data">{device.uptime}</span>
                  </td>
                  <td className="actions-column" onClick={(e) => e.stopPropagation()}>
                    <Button icon="eye-open" minimal small onClick={() => handleViewDevice(device)} title="View Details" />
                    <Button icon="edit" minimal small title="Edit Device" />
                    <Button
                      icon="trash"
                      minimal
                      small
                      intent="danger"
                      title="Delete Device"
                      loading={deleteDeviceMutation.isPending && deleteDeviceMutation.variables === device.id}
                      onClick={() => {
                        deleteDeviceMutation.mutate(device.id);
                      }}
                    />
                  </td>
                </tr>
              ))}
            </tbody>
          </HTMLTable>
        )}
      </Card>

      {/* Add Device Dialog */}
      <Dialog
        icon="add"
        title="Add Device"
        isOpen={isAddDialogOpen}
        onClose={() => setIsAddDialogOpen(false)}
      >
        <DialogBody>
          <FormGroup label="Name" labelInfo="(required)">
            <InputGroup
              placeholder="e.g. Temperature Sensor A1"
              value={newDevice.name}
              onChange={(e) => setNewDevice({ ...newDevice, name: e.target.value })}
            />
          </FormGroup>
          <FormGroup label="Device Type" labelInfo="(required)">
            <HTMLSelect
              fill
              value={newDevice.device_type_id || defaultTypeId}
              onChange={(e) => setNewDevice({ ...newDevice, device_type_id: Number(e.target.value) })}
            >
              {deviceTypes.map((dt) => (
                <option key={dt.id} value={dt.id}>
                  {dt.name}
                </option>
              ))}
            </HTMLSelect>
          </FormGroup>
          <FormGroup label="Fleet">
            <HTMLSelect
              fill
              value={newDevice.fleet_id ?? ''}
              onChange={(e) =>
                setNewDevice({
                  ...newDevice,
                  fleet_id: e.target.value ? Number(e.target.value) : undefined,
                })
              }
            >
              <option value="">No fleet</option>
              {fleets.map((f) => (
                <option key={f.id} value={f.id}>
                  {f.name}
                </option>
              ))}
            </HTMLSelect>
          </FormGroup>
          <FormGroup label="Location">
            <InputGroup
              placeholder="e.g. Building A, Floor 2"
              value={newDevice.location}
              onChange={(e) => setNewDevice({ ...newDevice, location: e.target.value })}
            />
          </FormGroup>
          <FormGroup label="Firmware">
            <InputGroup
              placeholder="e.g. v1.2.0"
              value={newDevice.firmware}
              onChange={(e) => setNewDevice({ ...newDevice, firmware: e.target.value })}
            />
          </FormGroup>
          {createDeviceMutation.isError && (
            <Callout intent="danger" icon="error">
              Failed to create device. Please try again.
            </Callout>
          )}
        </DialogBody>
        <DialogFooter
          actions={
            <>
              <Button onClick={() => setIsAddDialogOpen(false)}>Cancel</Button>
              <Button
                intent="primary"
                icon="add"
                onClick={handleAddDevice}
                loading={createDeviceMutation.isPending}
                disabled={!newDevice.name.trim()}
              >
                Add Device
              </Button>
            </>
          }
        />
      </Dialog>

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
                      {selectedDevice.device_type_name}
                      {selectedDevice.fleet_name && (
                        <>
                          <span className="banner-sep">|</span>
                          {selectedDevice.fleet_name}
                        </>
                      )}
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
                        Device offline — last seen {selectedDevice.last_seen}
                      </Callout>
                    )}

                    <div className="detail-grid">
                      <div className="detail-item">
                        <span className="section-label">Device ID</span>
                        <span className="detail-value mono-data">{selectedDevice.id}</span>
                      </div>
                      <div className="detail-item">
                        <span className="section-label">Type</span>
                        <span className="detail-value">{selectedDevice.device_type_name}</span>
                      </div>
                      <div className="detail-item">
                        <span className="section-label">Fleet</span>
                        <span className="detail-value">{selectedDevice.fleet_name ?? '—'}</span>
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
                        <span className="detail-value mono-data">{selectedDevice.last_seen}</span>
                      </div>
                      <div className="detail-item">
                        <span className="section-label">Uptime</span>
                        <span className="detail-value mono-data">{selectedDevice.uptime}</span>
                      </div>
                    </div>

                    <Divider style={{ margin: '16px 0' }} />

                    <span className="section-label">Quick Actions</span>
                    <div className="quick-actions">
                      <Button
                        icon="refresh"
                        fill
                        loading={restartDeviceMutation.isPending}
                        onClick={() => {
                          if (selectedDevice) restartDeviceMutation.mutate(selectedDevice.id);
                        }}
                      >
                        Restart
                      </Button>
                      <Button icon="cloud-upload" fill>Update FW</Button>
                      <Button icon="chart" fill>Telemetry</Button>
                      <Button icon="cog" fill>Configure</Button>
                    </div>
                  </>
                )}

                {drawerTab === 'telemetry' && (
                  <div className="telemetry-tab">
                    {telemetryChartData.length === 0 ? (
                      <Callout icon="info-sign" intent="primary">
                        No telemetry data available for this device.
                      </Callout>
                    ) : (
                      [
                        { label: 'Temperature', dataKey: 'temperature' as const, color: '#2965CC', unit: '\u00B0C' },
                        { label: 'Humidity', dataKey: 'humidity' as const, color: '#0F9960', unit: '%' },
                        { label: 'Battery Level', dataKey: 'battery' as const, color: '#D99E0B', unit: '%' },
                      ].map((metric) => {
                        const latestValue = telemetryChartData[telemetryChartData.length - 1]?.[metric.dataKey];
                        return (
                          <div key={metric.label} className="telemetry-chart">
                            <div className="telemetry-header">
                              <span className="section-label">{metric.label}</span>
                              <span className="mono-data" style={{ fontSize: 14, color: metric.color }}>
                                {latestValue != null ? `${latestValue}${metric.unit}` : '—'}
                              </span>
                            </div>
                            <ResponsiveContainer width="100%" height={60}>
                              <AreaChart data={telemetryChartData}>
                                <defs>
                                  <linearGradient id={`tel-${metric.label.replace(/\s/g, '')}`} x1="0" y1="0" x2="0" y2="1">
                                    <stop offset="0%" stopColor={metric.color} stopOpacity={0.3} />
                                    <stop offset="100%" stopColor={metric.color} stopOpacity={0} />
                                  </linearGradient>
                                </defs>
                                <Area
                                  type="monotone"
                                  dataKey={metric.dataKey}
                                  stroke={metric.color}
                                  strokeWidth={1.5}
                                  fill={`url(#tel-${metric.label.replace(/\s/g, '')})`}
                                  dot={false}
                                  isAnimationActive={false}
                                  connectNulls
                                />
                              </AreaChart>
                            </ResponsiveContainer>
                          </div>
                        );
                      })
                    )}
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
