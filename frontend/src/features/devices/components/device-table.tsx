import { memo, useMemo } from 'react';
import { Card, Checkbox, Elevation, H4, HTMLTable, Icon, Tag } from '@blueprintjs/core';
import { UPlotChart } from '../../../components/charts/UPlot';
import { toSparklineData, sparklineOpts } from '../../../components/charts/uplot-helpers';
import type { Device } from '../../../types/api';
import type { DeviceSortDir, DeviceSortField } from '../hooks/use-device-list-state';

interface DeviceTableProps {
  devices: Device[];
  sortField: DeviceSortField;
  sortDir: DeviceSortDir;
  activeRowIndex: number;
  getRowProps: (index: number) => {
    ref: (element: HTMLElement | null) => void;
    tabIndex: number;
    'data-roving-item': boolean;
    'data-keyboard-active': boolean | undefined;
    onFocus: () => void;
  };
  isSelected: (id: string) => boolean;
  onSort: (field: DeviceSortField) => void;
  onViewDevice: (device: Device) => void;
  onToggleDevice: (id: string) => void;
  onSelectAllVisible: (ids: string[]) => void;
  onDeselectAllVisible: () => void;
  onMouseEnter: (device: Device, event: React.MouseEvent<HTMLElement>) => void;
  onMouseLeave: () => void;
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
    <th className="sortable-th" onClick={() => onSort(field)}>
      <span className="th-content">
        {children}
        {sortField === field && (
          <Icon icon={sortDir === 'asc' ? 'chevron-up' : 'chevron-down'} size={12} />
        )}
      </span>
    </th>
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
  const plotData = useMemo(() => toSparklineData(values), [values]);
  const opts = useMemo(() => sparklineOpts(color, 0.15), [color]);
  return <UPlotChart options={opts} data={plotData} height={24} />;
});

export function DeviceTable({
  devices,
  sortField,
  sortDir,
  activeRowIndex,
  getRowProps,
  isSelected,
  onSort,
  onViewDevice,
  onToggleDevice,
  onSelectAllVisible,
  onDeselectAllVisible,
  onMouseEnter,
  onMouseLeave,
}: DeviceTableProps) {
  const allVisibleSelected = devices.length > 0 && devices.every((device) => isSelected(device.id));
  const someVisibleSelected =
    devices.some((device) => isSelected(device.id)) && !allVisibleSelected;

  return (
    <Card elevation={Elevation.ONE} className="devices-card">
      {devices.length === 0 ? (
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
              </th>
              <th style={{ width: 40 }} />
              <SortHeader field="name" sortField={sortField} sortDir={sortDir} onSort={onSort}>
                Name
              </SortHeader>
              <th>Type</th>
              <th>Fleet</th>
              <SortHeader field="last_seen" sortField={sortField} sortDir={sortDir} onSort={onSort}>
                Last Seen
              </SortHeader>
              <th>Firmware</th>
              <th style={{ width: 80 }}>Activity</th>
              <th>Uptime</th>
            </tr>
          </thead>
          <tbody>
            {devices.map((device, index) => {
              const rowProps = getRowProps(index);
              return (
                <tr
                  key={device.id}
                  ref={rowProps.ref}
                  tabIndex={rowProps.tabIndex}
                  data-roving-item={rowProps['data-roving-item']}
                  data-keyboard-active={rowProps['data-keyboard-active']}
                  data-focus-region-initial={index === activeRowIndex ? 'true' : undefined}
                  aria-selected={isSelected(device.id)}
                  className={`device-row ${isSelected(device.id) ? 'device-row--selected' : ''}`}
                  onClick={() => onViewDevice(device)}
                  onFocus={rowProps.onFocus}
                  onMouseEnter={(e) => onMouseEnter(device, e)}
                  onMouseLeave={onMouseLeave}
                >
                  <td onClick={(e) => e.stopPropagation()}>
                    <Checkbox
                      checked={isSelected(device.id)}
                      onChange={() => onToggleDevice(device.id)}
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
                      <Tag minimal intent="primary">
                        {device.fleet_name}
                      </Tag>
                    ) : (
                      <span style={{ color: 'hsl(var(--muted))', fontSize: 12 }}>-</span>
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
                      <RowSparkline deviceId={device.id} color={getStatusColor(device.status)} />
                    </div>
                  </td>
                  <td>
                    <span className="mono-data">{device.uptime}</span>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </HTMLTable>
      )}
    </Card>
  );
}
