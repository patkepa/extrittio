import { useQuery } from '@tanstack/react-query';
import { useMemo } from 'react';
import client from '../api/client';
import { queryKeys } from './query-keys';
import type { Device, LocationPoint } from '../types/api';
import type { components } from '../types/openapi';
import { loadLocationBatches } from '../api/location-batches';

type DeviceLocation = components['schemas']['DeviceLocationResponse'];

export function useDeviceLocations(devices: readonly Device[]) {
  const identities = useMemo(
    () =>
      devices
        .map((device) => [device.id, device.blueprint_revision_id] as const)
        .sort(([a], [b]) => a.localeCompare(b)),
    [devices],
  );
  return useQuery({
    queryKey: queryKeys.locations.batch(identities),
    queryFn: ({ signal }) =>
      loadLocationBatches<DeviceLocation>(
        identities.map(([id]) => id),
        async (device_ids) => {
          signal.throwIfAborted();
          const { data } = await client.post<DeviceLocation[]>(
            '/devices/locations/latest',
            { device_ids },
            { signal },
          );
          return data;
        },
      ),
    enabled: identities.length > 0,
    retry: false,
    staleTime: 0,
    refetchInterval: 10_000,
  });
}

export function useLatestLocation(deviceId: string | null) {
  return useQuery({
    queryKey: queryKeys.locations.latest(deviceId!),
    queryFn: async () => {
      const { data } = await client.get<LocationPoint | null>(
        `/devices/${deviceId}/location/latest`,
      );
      return data;
    },
    enabled: !!deviceId,
    staleTime: 10_000,
  });
}
