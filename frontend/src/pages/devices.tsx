import { useEffect, useMemo, useState, memo } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import {
  Card,
  Checkbox,
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
} from '@blueprintjs/core';
import { AreaChart, Area, ResponsiveContainer } from 'recharts';
import { useDevices } from '../hooks/use-devices';
import { useFleets } from '../hooks/use-fleets';
import { useUIStore } from '../stores/ui-store';
import { AddDeviceDialog } from '../components/devices/add-device-dialog';
import { useDeviceHoverTooltip, DeviceHoverTooltip } from '../components/devices/device-hover-tooltip';
import { useSelectionStore } from '../stores/selection-store';
import { BulkActionBar } from '../components/devices/bulk-action-bar';
import type { Device, BulkDeviceFilters, ListDevicesParams } from '../types/api';
import './devices.css';

type SortField = 'name' | 'status' | 'last_seen' | 'uptime';
type SortDir = 'asc' | 'desc';

const SortHeader = memo(({ field, sortField, sortDir, onSort, children }: {
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
));

const sparklineCache = new Map<string, { v: number; i: number }[]>();

function getSparklineData(id: string): { v: number; i: number }[] {
  let cached = sparklineCache.get(id);
  if (cached) return cached;
  let hash = 0;
  for (const ch of id) hash = ((hash << 5) - hash + ch.charCodeAt(0)) | 0;
  cached = Array.from({ length: 7 }, (_, i) => ({ v: Math.abs((hash * (i + 1)) % 100), i }));
  sparklineCache.set(id, cached);
  return cached;
}

export const Devices = () => {
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();
  const [searchQuery, setSearchQuery] = useState('');
  const [filterStatus, setFilterStatus] = useState<string>('all');
  const [sortField, setSortField] = useState<SortField>('name');
  const [sortDir, setSortDir] = useState<SortDir>('asc');
  const { openAddDeviceDialog } = useUIStore();

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

  // Selection store
  const {
    selectedDeviceIds,
    isAllMatchingSelected,
    toggleDevice,
    selectAllVisible,
    deselectAllVisible,
    clearSelection,
    isSelected,
  } = useSelectionStore();

  const hasSelection = selectedDeviceIds.size > 0 || isAllMatchingSelected;

  // Build server-side filter params for bulk targeting
  const currentFilters: BulkDeviceFilters = {
    ...(filterStatus !== 'all' ? { status: filterStatus } : {}),
    ...(searchQuery ? { search: searchQuery } : {}),
    ...(filterFleetId ? { fleet_id: filterFleetId } : {}),
  };

  // Clear selection when filters change
  useEffect(() => {
    clearSelection();
  }, [filterStatus, searchQuery, filterFleetId, clearSelection]);

  // Search uses a debounced value to avoid firing a request per keystroke
  const [debouncedSearch, setDebouncedSearch] = useState('');
  useEffect(() => {
    const timer = setTimeout(() => setDebouncedSearch(searchQuery), 300);
    return () => clearTimeout(timer);
  }, [searchQuery]);

  const queryParams: ListDevicesParams = {
    ...(filterFleetId ? { fleet_id: filterFleetId } : {}),
    ...(debouncedSearch ? { search: debouncedSearch } : {}),
  };
  const devicesQuery = useDevices(
    Object.keys(queryParams).length > 0 ? queryParams : undefined
  );
  const devices = devicesQuery.data?.data ?? [];
  const totalDeviceCount = devicesQuery.data?.total ?? 0;
  const isLoading = devicesQuery.isLoading;
  const error = devicesQuery.error;

  const { data: fleets = [] } = useFleets();
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

  // Status filtering stays client-side so we can show pill counts
  const filteredDevices = devices
    .filter((device) => filterStatus === 'all' || device.status === filterStatus)
    .sort((a, b) => {
      const dir = sortDir === 'asc' ? 1 : -1;
      if (sortField === 'name') return a.name.localeCompare(b.name) * dir;
      if (sortField === 'status') return a.status.localeCompare(b.status) * dir;
      if (sortField === 'last_seen') return (a.last_seen ?? '').localeCompare(b.last_seen ?? '') * dir;
      if (sortField === 'uptime') return (a.uptime ?? '').localeCompare(b.uptime ?? '') * dir;
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

  const statusCounts = useMemo(() => ({
    all: devices.length,
    online: devices.filter((d) => d.status === 'online').length,
    offline: devices.filter((d) => d.status === 'offline').length,
  }), [devices]);

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

      {/* Filters and Search / Bulk Action Bar */}
      {hasSelection ? (
        <Card elevation={Elevation.ONE} className="devices-controls">
          <BulkActionBar
            totalMatchingCount={totalDeviceCount}
            visibleCount={filteredDevices.length}
            currentFilters={currentFilters}
          />
        </Card>
      ) : (
        <Card elevation={Elevation.ONE} className="devices-controls">
          <div className="controls-row">
            <div className="search-section">
              <InputGroup
                leftIcon="search"
                placeholder="Search by name or type..."
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
      )}

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
                <th style={{ width: 40 }} onClick={(e) => e.stopPropagation()}>
                  <Checkbox
                    checked={
                      filteredDevices.length > 0 &&
                      filteredDevices.every((d) => isSelected(d.id))
                    }
                    indeterminate={
                      filteredDevices.some((d) => isSelected(d.id)) &&
                      !filteredDevices.every((d) => isSelected(d.id))
                    }
                    onChange={() => {
                      if (filteredDevices.every((d) => isSelected(d.id))) {
                        deselectAllVisible();
                      } else {
                        selectAllVisible(filteredDevices.map((d) => d.id));
                      }
                    }}
                    style={{ marginBottom: 0 }}
                  />
                </th>
                <th style={{ width: 40 }}></th>
                <SortHeader field="name" sortField={sortField} sortDir={sortDir} onSort={handleSort}>Name</SortHeader>
                <th>Type</th>
                <th>Fleet</th>
                <th>Last Seen</th>
                <th>Firmware</th>
                <th style={{ width: 80 }}>Activity</th>
                <th>Uptime</th>
              </tr>
            </thead>
            <tbody>
              {filteredDevices.map((device, idx) => (
                  <tr
                    key={device.id}
                    className={`device-row ${isSelected(device.id) ? 'device-row--selected' : ''}`}
                    onClick={() => handleViewDevice(device)}
                    onMouseEnter={(e) => onMouseEnter(device, e)}
                    onMouseLeave={onMouseLeave}
                    style={{ animationDelay: `${idx * 30}ms` }}
                  >
                    <td onClick={(e) => e.stopPropagation()}>
                      <Checkbox
                        checked={isSelected(device.id)}
                        onChange={() => toggleDevice(device.id)}
                        style={{ marginBottom: 0 }}
                      />
                    </td>
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
                    <td>
                      <span className="mono-data">{device.last_seen}</span>
                    </td>
                    <td>
                      <code className="firmware-badge">{device.firmware}</code>
                    </td>
                    <td>
                      <div className="row-sparkline">
                        <ResponsiveContainer width="100%" height={24}>
                          <AreaChart data={getSparklineData(device.id)}>
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
                  </tr>
              ))}
            </tbody>
          </HTMLTable>
        )}
      </Card>

      {/* Hover Summary Card */}
      <DeviceHoverTooltip device={hoveredDevice} position={hoverPos} />

      <AddDeviceDialog />
    </div>
  );
};
