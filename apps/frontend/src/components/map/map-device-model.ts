import type { Device, LocationPoint } from '../../types/api';

export interface MapDevice {
  id: string;
  name: string;
  status: string;
  last_seen_at?: string | null;
  location: LocationPoint;
}

export function locatedMapDevices(
  devices: readonly Pick<Device, 'id' | 'name' | 'status' | 'last_seen_at'>[],
  locations: readonly { device_id: string; location: LocationPoint }[],
): MapDevice[] {
  const byId = new Map(devices.map((device) => [device.id, device]));
  return locations.flatMap(({ device_id, location }) => {
    const device = byId.get(device_id);
    if (
      !device ||
      !Number.isFinite(location.latitude) ||
      !Number.isFinite(location.longitude) ||
      Math.abs(location.latitude) > 90 ||
      Math.abs(location.longitude) > 180
    )
      return [];
    return [
      {
        id: device.id,
        name: device.name,
        status: device.status,
        last_seen_at: device.last_seen_at,
        location,
      },
    ];
  });
}
