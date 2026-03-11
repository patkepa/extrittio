import { useEffect, useState } from 'react';
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
  Alert,
} from '@blueprintjs/core';
import { AreaChart, Area, ResponsiveContainer } from 'recharts';
import { useDevices, useDeleteDevice } from '../hooks/use-devices';
import { useFleets } from '../hooks/use-fleets';
import { useUIStore } from '../stores/ui-store';
import { AddDeviceDialog } from '../components/devices/add-device-dialog';
import { useDeviceHoverTooltip, DeviceHoverTooltip } from '../components/devices/device-hover-tooltip';
import type { Device } from '../types/api';
import { showSuccessToast, showErrorToast } from '../utils/toaster';
import './devices.css';

type SortField = 'name' | 'status' | 'last_seen' | 'uptime';
type SortDir = 'asc' | 'desc';

const SortHeader = ({ field, sortField, sortDir, onSort, children }: {
  field: SortField;
  sortField: SortField;
  sortDir: SortDir;
  onSort: (field: SortField) => void;
  children: React.ReactNode;
}) => (
  <th className="sortable-th" onClick={() => onSort(field)}>
    <span className="th-content">
      {children}
      {sortField === field && (
        <Icon icon={sortDir === 'asc' ? 'chevron-up' : 'chevron-down'} size={12} />
      )}
    </span>
  </th>
);

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
  const { openAddDeviceDialog } = useUIStore();

  // Delete confirmation state
  const [deviceToDelete, setDeviceToDelete] = useState<Device | null>(null);

  // Hover tooltip
  const { hoveredDevice, hoverPos, onMouseEnter, onMouseLeave } = useDeviceHoverTooltip();

  // Fleet filter from URL query param
  const filterFleetId = searchParams.get('fleet_id') ? Number(searchParams.get('fleet_id')) : null;

  const setFilterFleetId = (fleetId: number | null) => {
    setSearchParams((prev) => {
      const next = new URLSearchParams(prev);
      if (fleetId === null) {
        next.delete('fleet_id');
      } else {
        next.set('fleet_id', String(fleetId));
      }
      return next;
    });
  };

  const { data: devices = [], isLoading, error } = useDevices(
    filterFleetId ? { fleet_id: filterFleetId } : undefined
  );
  const { data: fleets = [] } = useFleets();
  const deleteDeviceMutation = useDeleteDevice();

  // Navigate to device detail when accessed with ?device= query param (from command palette)
  const deviceParam = searchParams.get('device');
  useEffect(() => {
    if (deviceParam && devices.length > 0) {
      const device = devices.find((d) => d.id === deviceParam);
      if (device) {
        navigate(`/devices/${device.id}`, { replace: true });
      } else {
        setSearchParams((prev) => {
          const next = new URLSearchParams(prev);
          next.delete('device');
          return next;
        }, { replace: true });
      }
    }
  }, [deviceParam, devices, navigate, setSearchParams]);

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

  const statusCounts = {
    all: devices.length,
    online: devices.filter((d) => d.status === 'online').length,
    offline: devices.filter((d) => d.status === 'offline').length,
  };

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
                <SortHeader field="name" sortField={sortField} sortDir={sortDir} onSort={handleSort}>Name</SortHeader>
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
                    onMouseEnter={(e) => onMouseEnter(device, e)}
                    onMouseLeave={onMouseLeave}
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
                        onClick={() => setDeviceToDelete(device)}
                      />
                    </td>
                  </tr>
              ))}
            </tbody>
          </HTMLTable>
        )}
      </Card>

      {/* Hover Summary Card */}
      <DeviceHoverTooltip device={hoveredDevice} position={hoverPos} />

      {/* Delete Confirmation */}
      <Alert
        isOpen={deviceToDelete !== null}
        icon="trash"
        intent="danger"
        confirmButtonText="Delete"
        cancelButtonText="Cancel"
        onConfirm={() => {
          if (deviceToDelete) {
            deleteDeviceMutation.mutate(deviceToDelete.id, {
              onSuccess: () => {
                setDeviceToDelete(null);
                void showSuccessToast('Device deleted');
              },
              onError: () => {
                setDeviceToDelete(null);
                void showErrorToast('Failed to delete device');
              },
            });
          }
        }}
        onCancel={() => setDeviceToDelete(null)}
      >
        <p>Are you sure you want to delete <strong>{deviceToDelete?.name}</strong>? This action cannot be undone.</p>
      </Alert>

      <AddDeviceDialog />
    </div>
  );
};
