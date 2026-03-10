import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { useNavigate, useSearchParams } from 'react-router-dom';
import {
  Card,
  Elevation,
  H3,
  HTMLTable,
  Tag,
  Button,
  InputGroup,
  Icon,
  H4,
  Callout,
  Spinner,
  Dialog,
  DialogBody,
  DialogFooter,
  FormGroup,
  HTMLSelect,
} from '@blueprintjs/core';
import { AreaChart, Area, ResponsiveContainer } from 'recharts';
import { useDevices, useCreateDevice, useDeleteDevice } from '../hooks/use-devices';
import { useDeviceTypes } from '../hooks/use-device-types';
import { useFleets } from '../hooks/use-fleets';
import { useUIStore } from '../stores/ui-store';
import { DeviceSummaryCard } from '../components/devices/device-summary-card';
import type { Device } from '../types/api';
import './devices.css';

type SortField = 'name' | 'status' | 'last_seen' | 'uptime';
type SortDir = 'asc' | 'desc';

function generateSparkline(id: string): number[] {
  let hash = 0;
  for (const ch of id) hash = ((hash << 5) - hash + ch.charCodeAt(0)) | 0;
  return Array.from({ length: 7 }, (_, i) => Math.abs((hash * (i + 1)) % 100));
}

export const Devices = () => {
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();
  const [searchQuery, setSearchQuery] = useState('');
  const [filterStatus, setFilterStatus] = useState<string>('all');
  const [sortField, setSortField] = useState<SortField>('name');
  const [sortDir, setSortDir] = useState<SortDir>('asc');
  const { isAddDeviceDialogOpen: isAddDialogOpen, openAddDeviceDialog, closeAddDeviceDialog } = useUIStore();
  const [newDevice, setNewDevice] = useState({
    name: '',
    device_type_id: 0,
    fleet_id: undefined as number | undefined,
    location: '',
    firmware: '',
  });

  // Hover tooltip state
  const [hoveredDevice, setHoveredDevice] = useState<Device | null>(null);
  const [hoverPos, setHoverPos] = useState({ x: 0, y: 0 });
  const hoverTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Cleanup hover timeout on unmount
  useEffect(() => {
    return () => {
      if (hoverTimeoutRef.current) clearTimeout(hoverTimeoutRef.current);
    };
  }, []);

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

  // Navigate to device detail when accessed with ?device= query param (from command palette)
  const deviceParam = searchParams.get('device');
  useEffect(() => {
    if (deviceParam && devices.length > 0) {
      const device = devices.find((d) => d.id === deviceParam);
      if (device) {
        navigate(`/devices/${device.id}`, { replace: true });
      }
      searchParams.delete('device');
      setSearchParams(searchParams, { replace: true });
    }
  }, [deviceParam, devices, navigate, searchParams, setSearchParams]);

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
    navigate(`/devices/${device.id}`);
  };

  const handleRowMouseEnter = (device: Device, e: React.MouseEvent) => {
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    setHoverPos({ x: rect.left - 8, y: rect.top + rect.height / 2 });
    hoverTimeoutRef.current = setTimeout(() => {
      setHoveredDevice(device);
    }, 300);
  };

  const handleRowMouseLeave = () => {
    if (hoverTimeoutRef.current) {
      clearTimeout(hoverTimeoutRef.current);
      hoverTimeoutRef.current = null;
    }
    setHoveredDevice(null);
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
        <Button intent="primary" icon="add" onClick={() => openAddDeviceDialog()}>
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
                    className="device-row"
                    onClick={() => handleViewDevice(device)}
                    onMouseEnter={(e) => handleRowMouseEnter(device, e)}
                    onMouseLeave={handleRowMouseLeave}
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

      {/* Hover Summary Card */}
      {hoveredDevice && createPortal(
        <div
          className="device-hover-tooltip"
          style={{
            position: 'fixed',
            left: hoverPos.x,
            top: hoverPos.y,
            transform: 'translate(-100%, -50%)',
            zIndex: 30,
          }}
        >
          <DeviceSummaryCard device={hoveredDevice} />
        </div>,
        document.body
      )}

      {/* Add Device Dialog */}
      <Dialog
        icon="add"
        title="Add Device"
        isOpen={isAddDialogOpen}
        onClose={() => closeAddDeviceDialog()}
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
              <Button onClick={() => closeAddDeviceDialog()}>Cancel</Button>
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
    </div>
  );
};
