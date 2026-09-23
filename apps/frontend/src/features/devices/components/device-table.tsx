import { memo, useEffect, useMemo, useState } from 'react';
import { List } from 'react-window';
import { Button, Card, Checkbox, Elevation, HTMLSelect, Icon, Tag } from '@blueprintjs/core';
import type { Device } from '../../../types/api';
import { EmptyState, StatusLed } from '@patkepa/kantzen-ui';
import type { DeviceSortDir, DeviceSortField } from '../hooks/use-device-list-state';
import { BlueprintTag } from '../../../components/devices/blueprint-tag';

interface DeviceTableProps {
  devices: Device[];
  canSelect: boolean;
  selectedDeviceIds: Set<string>;
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
  onSort: (field: DeviceSortField) => void;
  onViewDevice: (device: Device) => void;
  onToggleDevice: (id: string) => void;
  onSelectAllVisible: (ids: string[]) => void;
  onDeselectAllVisible: (ids: string[]) => void;
}

interface DeviceRowProps {
  devices: Device[];
  canSelect: boolean;
  activeRowIndex: number;
  getRowProps: DeviceTableProps['getRowProps'];
  registerRow: DeviceTableProps['registerRow'];
  selectedDeviceIds: Set<string>;
  onViewDevice: (device: Device) => void;
  onToggleDevice: (id: string) => void;
}

const ROW_HEIGHT = 58;
const MOBILE_ROW_HEIGHT = 94;

function useMobileLayout() {
  const [mobile, setMobile] = useState(() => window.matchMedia('(max-width: 768px)').matches);
  useEffect(() => {
    const query = window.matchMedia('(max-width: 768px)');
    const update = () => setMobile(query.matches);
    query.addEventListener('change', update);
    return () => query.removeEventListener('change', update);
  }, []);
  return mobile;
}

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
      aria-sort={sortField === field ? (sortDir === 'asc' ? 'ascending' : 'descending') : 'none'}
    >
      <button type="button" className="th-content sort-button" onClick={() => onSort(field)}>
        {children}
        {sortField === field && (
          <Icon icon={sortDir === 'asc' ? 'chevron-up' : 'chevron-down'} size={12} />
        )}
      </button>
    </div>
  ),
);

function DeviceRow({
  index,
  style,
  devices,
  canSelect,
  activeRowIndex,
  getRowProps,
  registerRow,
  selectedDeviceIds,
  onViewDevice,
  onToggleDevice,
}: {
  index: number;
  style: React.CSSProperties;
} & DeviceRowProps) {
  const device = devices[index];
  if (!device) return null;

  const focusProps = getRowProps(index);
  const selected = selectedDeviceIds.has(device.id);

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
      {canSelect && (
        <div
          role="cell"
          className="devices-grid-cell device-cell-select"
          onClick={(e) => e.stopPropagation()}
        >
          <Checkbox
            aria-label={`Select ${device.name}`}
            checked={selected}
            onChange={() => onToggleDevice(device.id)}
            style={{ marginBottom: 0 }}
          />
        </div>
      )}
      <div role="cell" className="devices-grid-cell device-cell-status">
        <StatusLed status={device.status} />
      </div>
      <div role="cell" className="devices-grid-cell device-cell-name">
        <div className="device-name-cell">
          <strong>{device.name}</strong>
          <span className="device-id mono-data">{device.id}</span>
        </div>
      </div>
      <div role="cell" className="devices-grid-cell device-cell-blueprint">
        <BlueprintTag
          name={device.blueprint_name}
          icon={device.blueprint_icon}
          colorHex={device.blueprint_color}
        />
      </div>
      <div role="cell" className="devices-grid-cell device-cell-fleet">
        {device.fleet_name ? (
          <Tag minimal intent="primary">
            {device.fleet_name}
          </Tag>
        ) : (
          <span style={{ color: 'hsl(var(--muted))', fontSize: 12 }}>-</span>
        )}
      </div>
      <div role="cell" className="devices-grid-cell device-cell-last-seen">
        <span className="mono-data">{device.last_seen}</span>
      </div>
      <div role="cell" className="devices-grid-cell device-cell-firmware">
        <code className="firmware-badge">{device.firmware}</code>
      </div>
      <div role="cell" className="devices-grid-cell device-cell-uptime">
        <span className="mono-data">{device.uptime}</span>
      </div>
    </div>
  );
}

export function DeviceTable({
  devices,
  canSelect,
  selectedDeviceIds,
  sortField,
  sortDir,
  activeRowIndex,
  getRowProps,
  registerRow,
  onSort,
  onViewDevice,
  onToggleDevice,
  onSelectAllVisible,
  onDeselectAllVisible,
}: DeviceTableProps) {
  const mobile = useMobileLayout();
  const allVisibleSelected =
    devices.length > 0 && devices.every((device) => selectedDeviceIds.has(device.id));
  const someVisibleSelected =
    devices.some((device) => selectedDeviceIds.has(device.id)) && !allVisibleSelected;
  const rowProps = useMemo<DeviceRowProps>(
    () => ({
      devices,
      canSelect,
      activeRowIndex,
      getRowProps,
      registerRow,
      selectedDeviceIds,
      onViewDevice,
      onToggleDevice,
    }),
    [
      activeRowIndex,
      canSelect,
      devices,
      getRowProps,
      onToggleDevice,
      onViewDevice,
      registerRow,
      selectedDeviceIds,
    ],
  );

  return (
    <Card elevation={Elevation.ONE} className="devices-card">
      <div className="devices-mobile-sort">
        <HTMLSelect
          aria-label="Sort devices"
          value={sortField}
          onChange={(event) => onSort(event.target.value as DeviceSortField)}
          options={[
            { value: 'last_seen', label: 'Last seen' },
            { value: 'name', label: 'Name' },
            { value: 'status', label: 'Status' },
            { value: 'uptime', label: 'Uptime' },
          ]}
        />
        <Button
          minimal
          icon={sortDir === 'asc' ? 'sort-asc' : 'sort-desc'}
          onClick={() => onSort(sortField)}
          aria-label={sortDir === 'asc' ? 'Sort descending' : 'Sort ascending'}
        />
      </div>
      {devices.length === 0 ? (
        <EmptyState
          icon="search"
          title="No devices found"
          description="Try adjusting your search or filter criteria"
        />
      ) : (
        <div
          className={`devices-grid ${canSelect ? '' : 'devices-grid--read-only'}`}
          role="grid"
          aria-rowcount={devices.length}
        >
          <div className="devices-grid-header" role="row">
            {canSelect && (
              <div
                role="columnheader"
                className="devices-grid-cell"
                onClick={(e) => e.stopPropagation()}
              >
                <Checkbox
                  aria-label="Select all displayed devices"
                  checked={allVisibleSelected}
                  indeterminate={someVisibleSelected}
                  onChange={() => {
                    if (allVisibleSelected) {
                      onDeselectAllVisible(devices.map((device) => device.id));
                    } else {
                      onSelectAllVisible(devices.map((d) => d.id));
                    }
                  }}
                  style={{ marginBottom: 0 }}
                />
              </div>
            )}
            <div role="columnheader" className="devices-grid-cell" />
            <SortHeader field="name" sortField={sortField} sortDir={sortDir} onSort={onSort}>
              Name
            </SortHeader>
            <div role="columnheader" className="devices-grid-cell">
              Blueprint
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
              Uptime
            </div>
          </div>
          <List<DeviceRowProps>
            className="devices-virtual-list"
            rowComponent={DeviceRow}
            rowCount={devices.length}
            rowHeight={mobile ? MOBILE_ROW_HEIGHT : ROW_HEIGHT}
            rowProps={rowProps}
            overscanCount={8}
            style={{ height: 'min(68vh, 820px)', width: '100%' }}
          />
        </div>
      )}
    </Card>
  );
}
