export type DeviceSortField = 'name' | 'status' | 'last_seen' | 'uptime';
export type DeviceSortDir = 'asc' | 'desc';
export type DeviceStatusFilter = 'all' | 'online' | 'offline';

interface DeviceListItem {
  name: string;
  status: string;
  last_seen_at?: string | null;
  uptime?: string | null;
}

export function filterAndSortDevices<T extends DeviceListItem>(
  devices: readonly T[],
  statusFilter: DeviceStatusFilter,
  sortField: DeviceSortField,
  sortDirection: DeviceSortDir,
): T[] {
  const direction = sortDirection === 'asc' ? 1 : -1;
  return devices
    .filter((device) => statusFilter === 'all' || device.status === statusFilter)
    .sort((a, b) => {
      if (sortField === 'name') return a.name.localeCompare(b.name) * direction;
      if (sortField === 'status') return a.status.localeCompare(b.status) * direction;
      if (sortField === 'last_seen') {
        return (a.last_seen_at ?? '').localeCompare(b.last_seen_at ?? '') * direction;
      }
      return (a.uptime ?? '').localeCompare(b.uptime ?? '') * direction;
    });
}

export function countDeviceStatuses(devices: readonly DeviceListItem[]) {
  return devices.reduce(
    (counts, device) => {
      counts.all += 1;
      if (device.status === 'online') counts.online += 1;
      if (device.status === 'offline') counts.offline += 1;
      return counts;
    },
    { all: 0, online: 0, offline: 0 },
  );
}
