import type { DeviceContract, DeviceMetric } from '../../types/api';

export interface LocationBinding {
  stream: string;
  latitudePath: string;
  longitudePath: string;
  maxAgeMs: number;
  coordinateSystem: 'wgs84';
  unit: 'degrees';
}

export function locationBinding(contract: DeviceContract): LocationBinding | undefined {
  const document = contract.document as { location?: LocationBinding };
  const binding = document.location;
  return binding?.coordinateSystem === 'wgs84' && binding.unit === 'degrees' ? binding : undefined;
}

export interface LocationObservation {
  latitude: number;
  longitude: number;
  occurredAt: string;
  eventId: string;
}

export function locationObservations(
  contract: DeviceContract,
  metrics: DeviceMetric[],
): LocationObservation[] {
  const binding = locationBinding(contract);
  if (!binding) return [];
  const events = new Map<
    string,
    { occurredAt: string; latitude?: number; longitude?: number; invalid: boolean }
  >();
  for (const metric of metrics) {
    if (
      metric.contract_id !== contract.id ||
      metric.device_id !== contract.device_id ||
      metric.stream_key !== binding.stream
    )
      continue;
    if (metric.field_path !== binding.latitudePath && metric.field_path !== binding.longitudePath)
      continue;
    const event = events.get(metric.event_id) ?? { occurredAt: metric.occurred_at, invalid: false };
    if (
      event.occurredAt !== metric.occurred_at ||
      typeof metric.value !== 'number' ||
      !Number.isFinite(metric.value)
    ) {
      event.invalid = true;
    } else if (metric.field_path === binding.latitudePath) {
      if (event.latitude !== undefined && event.latitude !== metric.value) event.invalid = true;
      event.latitude = metric.value;
    } else {
      if (event.longitude !== undefined && event.longitude !== metric.value) event.invalid = true;
      event.longitude = metric.value;
    }
    events.set(metric.event_id, event);
  }
  return [...events]
    .flatMap(([eventId, event]) => {
      const { latitude, longitude, occurredAt } = event;
      if (
        event.invalid ||
        latitude === undefined ||
        longitude === undefined ||
        Math.abs(latitude) > 90 ||
        Math.abs(longitude) > 180 ||
        !Number.isFinite(Date.parse(occurredAt))
      )
        return [];
      return [{ eventId, occurredAt, latitude, longitude }];
    })
    .sort(
      (a, b) =>
        Date.parse(b.occurredAt) - Date.parse(a.occurredAt) || b.eventId.localeCompare(a.eventId),
    );
}
