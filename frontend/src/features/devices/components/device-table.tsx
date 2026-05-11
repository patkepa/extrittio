import { memo, useMemo } from 'react';
import { List } from 'react-window';
import { Card, Checkbox, Elevation, Icon, Tag } from '@blueprintjs/core';
import type { Device } from '../../../types/api';
import { EmptyState, StatusLed } from '@patkepa/ui';
import type { DeviceSortDir, DeviceSortField } from '../hooks/use-device-list-state';
import { DeviceTypeTag } from '../../../components/devices/device-type-tag';

interface DeviceTableProps {
  devices: Device[];
  sortField: DeviceSortField;
  sortDir: DeviceSortDir;
  activeRowIndex: number;
  getRowProps: (index: number) => {
    tabIndex: number;
    'data-roving-item': boolean;
    'data-keyboard-active': boolean | undefined;
    onFocus: () => void;
  };
  registerRow: (index: number, element: HTMLElement | null) => void;
  isSelected: (id: string) => boolean;
  onSort: (field: DeviceSortField) => void;
  onViewDevice: (device: Device) => void;
  onToggleDevice: (id: string) => void;
  onSelectAllVisible: (ids: string[]) => void;
  onDeselectAllVisible: () => void;
}

interface DeviceRowProps {
  devices: Device[];
  activeRowIndex: number;
  getRowProps: DeviceTableProps['getRowProps'];
  registerRow: DeviceTableProps['registerRow'];
  isSelected: (id: string) => boolean;
  onViewDevice: (device: Device) => void;
  onToggleDevice: (id: string) => void;
}

const ROW_HEIGHT = 58;

const SortHeader = memo(
  ({
    field,
    sortField,
    sortDir,
    onSort,
    children,
  }: {
    field: DeviceSortField;
    sortField: DeviceSortField;
    sortDir: DeviceSortDir;
    onSort: (field: DeviceSortField) => void;
    children: React.ReactNode;
  }) => (
    <div
      role="columnheader"
      className="devices-grid-cell sortable-th"
      onClick={() => onSort(field)}
    >
      <span className="th-content">
        {children}
        {sortField === field && (
          <Icon icon={sortDir === 'asc' ? 'chevron-up' : 'chevron-down'} size={12} />
        )}
      </span>
    </div>
  ),
);

const sparklineCache = new Map<string, number[]>();

function getSparklineValues(id: string): number[] {
  let cached = sparklineCache.get(id);
  if (cached) return cached;
  let hash = 0;
  for (const ch of id) hash = ((hash << 5) - hash + ch.charCodeAt(0)) | 0;
  cached = Array.from({ length: 7 }, (_, i) => Math.abs((hash * (i + 1)) % 100));
  sparklineCache.set(id, cached);
  return cached;
}

function getStatusColor(status: string) {
  switch (status) {
    case 'online':
      return '#0F9960';
    case 'offline':
      return '#E76A6E';
    default:
      return '#888';
  }
}

const RowSparkline = memo(({ deviceId, color }: { deviceId: string; color: string }) => {
  const values = getSparklineValues(deviceId);
  const points = useMemo(() => {
    const max = Math.max(...values, 1);
    return values
      .map((value, index) => {
        const x = (index / Math.max(values.length - 1, 1)) * 76 + 2;
        const y = 22 - (value / max) * 18;
        return `${x.toFixed(1)},${y.toFixed(1)}`;
      })
      .join(' ');
  }, [values]);

  return (
    <svg
      className="row-sparkline-svg"
      viewBox="0 0 80 24"
      role="img"
      aria-label="Device activity"
      focusable="false"
    >
      <polyline points={points} fill="none" stroke={color} strokeWidth="1.5" />
    </svg>
  );
});

function DeviceRow({
  index,
  style,
  devices,
  activeRowIndex,
  getRowProps,
  registerRow,
  isSelected,
  onViewDevice,
  onToggleDevice,
}: {
  index: number;
  style: React.CSSProperties;
} & DeviceRowProps) {
  const device = devices[index];
  if (!device) return null;

  const focusProps = getRowProps(index);
  const selected = isSelected(device.id);

  return (
    <div
      ref={(element) => registerRow(index, element)}
      style={style}
      role="row"
      tabIndex={focusProps.tabIndex}
      data-roving-item={focusProps['data-roving-item']}
      data-keyboard-active={focusProps['data-keyboard-active']}
      data-focus-region-initial={index === activeRowIndex ? 'true' : undefined}
      aria-selected={selected}
      className={`device-row devices-grid-row ${selected ? 'device-row--selected' : ''}`}
      onClick={() => onViewDevice(device)}
      onFocus={focusProps.onFocus}
    >
      <div role="cell" className="devices-grid-cell" onClick={(e) => e.stopPropagation()}>
        <Checkbox
          checked={selected}
          onChange={() => onToggleDevice(device.id)}
          style={{ marginBottom: 0 }}
        />
      </div>
      <div role="cell" className="devices-grid-cell">
        <StatusLed status={device.status} />
      </div>
      <div role="cell" className="devices-grid-cell">
        <div className="device-name-cell">
          <strong>{device.name}</strong>
          <span className="device-id mono-data">{device.id}</span>
        </div>
      </div>
      <div role="cell" className="devices-grid-cell">
        <DeviceTypeTag
          name={device.device_type_name}
          icon={device.device_type_icon}
          colorHex={device.device_type_color_hex}
        />
      </div>
      <div role="cell" className="devices-grid-cell">
        {device.fleet_name ? (
          <Tag minimal intent="primary">
            {device.fleet_name}
          </Tag>
        ) : (
          <span style={{ color: 'hsl(var(--muted))', fontSize: 12 }}>-</span>
        )}
      </div>
      <div role="cell" className="devices-grid-cell">
        <span className="mono-data">{device.last_seen}</span>
      </div>
      <div role="cell" className="devices-grid-cell">
        <code className="firmware-badge">{device.firmware}</code>
      </div>
      <div role="cell" className="devices-grid-cell">
        <div className="row-sparkline">
          <RowSparkline deviceId={device.id} color={getStatusColor(device.status)} />
        </div>
      </div>
      <div role="cell" className="devices-grid-cell">
        <span className="mono-data">{device.uptime}</span>
      </div>
    </div>
  );
}

export function DeviceTable({
  devices,
  sortField,
  sortDir,
  activeRowIndex,
  getRowProps,
  registerRow,
  isSelected,
  onSort,
  onViewDevice,
  onToggleDevice,
  onSelectAllVisible,
  onDeselectAllVisible,
}: DeviceTableProps) {
  const allVisibleSelected = devices.length > 0 && devices.every((device) => isSelected(device.id));
  const someVisibleSelected =
    devices.some((device) => isSelected(device.id)) && !allVisibleSelected;
  const rowProps = useMemo<DeviceRowProps>(
    () => ({
      devices,
      activeRowIndex,
      getRowProps,
      registerRow,
      isSelected,
      onViewDevice,
      onToggleDevice,
    }),
    [activeRowIndex, devices, getRowProps, isSelected, onToggleDevice, onViewDevice, registerRow],
  );

  return (
    <Card elevation={Elevation.ONE} className="devices-card">
      {devices.length === 0 ? (
        <EmptyState
          icon="search"
          title="No devices found"
          description="Try adjusting your search or filter criteria"
        />
      ) : (
        <div className="devices-grid" role="grid" aria-rowcount={devices.length}>
          <div className="devices-grid-header" role="row">
            <div
              role="columnheader"
              className="devices-grid-cell"
              onClick={(e) => e.stopPropagation()}
            >
              <Checkbox
                checked={allVisibleSelected}
                indeterminate={someVisibleSelected}
                onChange={() => {
                  if (allVisibleSelected) {
                    onDeselectAllVisible();
                  } else {
                    onSelectAllVisible(devices.map((d) => d.id));
                  }
                }}
                style={{ marginBottom: 0 }}
              />
            </div>
            <div role="columnheader" className="devices-grid-cell" />
            <SortHeader field="name" sortField={sortField} sortDir={sortDir} onSort={onSort}>
              Name
            </SortHeader>
            <div role="columnheader" className="devices-grid-cell">
              Type
            </div>
            <div role="columnheader" className="devices-grid-cell">
              Fleet
            </div>
            <SortHeader field="last_seen" sortField={sortField} sortDir={sortDir} onSort={onSort}>
              Last Seen
            </SortHeader>
            <div role="columnheader" className="devices-grid-cell">
              Firmware
            </div>
            <div role="columnheader" className="devices-grid-cell">
              Activity
            </div>
            <div role="columnheader" className="devices-grid-cell">
              Uptime
            </div>
          </div>
          <List<DeviceRowProps>
            className="devices-virtual-list"
            rowComponent={DeviceRow}
            rowCount={devices.length}
            rowHeight={ROW_HEIGHT}
            rowProps={rowProps}
            overscanCount={8}
            style={{ height: 'min(68vh, 820px)', width: '100%' }}
          />
        </div>
      )}
    </Card>
  );
}
