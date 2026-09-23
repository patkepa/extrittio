import { useState, useMemo } from 'react';
import { ButtonGroup, Button, Spinner, Callout, Tag } from '@blueprintjs/core';
import { useDeviceMetrics } from '../../hooks/use-telemetry';
import { useZones } from '../../hooks/use-zones';
import { DeviceMap } from '../map/device-map';
import { DeviceMarker } from '../map/device-marker';
import { LocationTrail } from '../map/location-trail';
import { ZoneLayer } from '../map/zone-layer';
import { RANGES, computeSince } from './telemetry-ranges';
import type { RangeKey } from './telemetry-ranges';
import type { DeviceContract } from '../../types/api';
import { useDeviceContract } from '../../hooks/use-devices';
import { locationBinding, locationObservations } from './contract-location-model';
import './location-tab.css';

function formatValue(v: number | null | undefined, decimals = 4): string {
  if (v == null) return '—';
  return v.toFixed(decimals);
}

function formatTimestamp(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleString(undefined, {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  });
}

// ---------------------------------------------------------------------------
// Range options (without "All" which is unnecessary for a map trail)
// ---------------------------------------------------------------------------

const LOCATION_RANGE_KEYS: RangeKey[] = ['15m', '1h', '6h', '24h', '7d', '30d'];

// ---------------------------------------------------------------------------
// Zone membership helper (point-in-circle, point-in-polygon)
// ---------------------------------------------------------------------------

function pointInCircle(
  lat: number,
  lon: number,
  center: [number, number],
  radiusMeters: number,
): boolean {
  const R = 6371000;
  const dLat = ((lat - center[0]) * Math.PI) / 180;
  const dLon = ((lon - center[1]) * Math.PI) / 180;
  const a =
    Math.sin(dLat / 2) * Math.sin(dLat / 2) +
    Math.cos((center[0] * Math.PI) / 180) *
      Math.cos((lat * Math.PI) / 180) *
      Math.sin(dLon / 2) *
      Math.sin(dLon / 2);
  const c = 2 * Math.atan2(Math.sqrt(a), Math.sqrt(1 - a));
  return R * c <= radiusMeters;
}

function pointInPolygon(lat: number, lon: number, points: [number, number][]): boolean {
  let inside = false;
  for (let i = 0, j = points.length - 1; i < points.length; j = i++) {
    const pi = points[i]!;
    const pj = points[j]!;
    const xi = pi[0],
      yi = pi[1];
    const xj = pj[0],
      yj = pj[1];
    const intersect = yi > lon !== yj > lon && lat < ((xj - xi) * (lon - yi)) / (yj - yi) + xi;
    if (intersect) inside = !inside;
  }
  return inside;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

interface LocationTabProps {
  deviceId: string;
  deviceName: string;
  deviceStatus: string;
}

export const LocationTab = (props: LocationTabProps) => {
  const query = useDeviceContract(props.deviceId, { retry: false });
  if (query.isLoading) return <Spinner />;
  if (query.isError || !query.data)
    return <Callout intent="danger">Unable to load the assigned device contract.</Callout>;
  if (!locationBinding(query.data))
    return <Callout>This blueprint does not declare a location binding.</Callout>;
  return <ContractLocationTab {...props} contract={query.data} />;
};

const ContractLocationTab = ({
  deviceId,
  deviceName,
  deviceStatus,
  contract,
}: LocationTabProps & { contract: DeviceContract }) => {
  const [selectedRange, setSelectedRange] = useState<RangeKey>('1h');
  const rangeConfig = RANGES[selectedRange];
  const since = useMemo(() => computeSince(rangeConfig), [rangeConfig]);

  const {
    data: rawRecords = [],
    isLoading,
    isFetching,
    isError,
  } = useDeviceMetrics(deviceId, {
    stream_key: locationBinding(contract)!.stream,
    limit: rangeConfig.limit,
    since,
  });

  const { data: zones = [] } = useZones();

  // Filter to records with valid location data (backend returns DESC order, newest first)
  const locationRecords = useMemo(
    () => locationObservations(contract, rawRecords),
    [contract, rawRecords],
  );

  // Latest record (first in DESC order)
  const latest = locationRecords[0];

  // Trail: chronological order (oldest → newest) for the Polyline
  const trailPoints = useMemo(
    () =>
      locationRecords
        .slice()
        .reverse()
        .map((r) => ({
          latitude: r.latitude,
          longitude: r.longitude,
          timestamp: r.occurredAt,
        })),
    [locationRecords],
  );

  // Determine which zones the current position is inside
  const activeZones = useMemo(() => {
    if (!latest) return [];
    return zones.filter((zone) => {
      if (zone.geometry_type === 'circle') {
        const geo = zone.geometry_json as { center: [number, number]; radius_meters: number };
        return pointInCircle(latest.latitude, latest.longitude, geo.center, geo.radius_meters);
      } else {
        const geo = zone.geometry_json as { points: [number, number][] };
        return pointInPolygon(latest.latitude, latest.longitude, geo.points);
      }
    });
  }, [latest, zones]);

  // Map center: latest position or fallback
  const center: [number, number] = latest
    ? [latest.latitude, latest.longitude]
    : [52.2297, 21.0122];

  if (isLoading) return <Spinner />;

  if (isError) {
    return (
      <Callout intent="danger" icon="error">
        Failed to load telemetry data. Try refreshing the page.
      </Callout>
    );
  }

  return (
    <div className="location-tab">
      {/* Range selector */}
      <div className="location-tab-controls">
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          {isFetching && !isLoading && <Spinner size={16} />}
          <ButtonGroup>
            {LOCATION_RANGE_KEYS.map((key) => (
              <Button
                key={key}
                active={selectedRange === key}
                onClick={() => setSelectedRange(key)}
                small
              >
                {RANGES[key].label}
              </Button>
            ))}
          </ButtonGroup>
        </div>
      </div>

      {locationRecords.length === 0 ? (
        <div className="location-tab-empty">No location data in this time range.</div>
      ) : (
        <>
          {/* Map */}
          <div className="location-tab-map">
            <DeviceMap center={center} zoom={13}>
              <ZoneLayer zones={zones} />
              <LocationTrail points={trailPoints} />
              {latest && (
                <DeviceMarker
                  deviceId={deviceId}
                  deviceName={deviceName}
                  status={deviceStatus}
                  latitude={latest.latitude}
                  longitude={latest.longitude}
                  lastSeen={latest.occurredAt}
                />
              )}
            </DeviceMap>
          </div>

          {/* Info row */}
          <div className="location-tab-info">
            {/* Current values table */}
            <div className="location-tab-values">
              <div className="telemetry-section">
                <span className="section-label">Last Recorded Position</span>
                <div className="telemetry-current-table">
                  <div className="telemetry-current-row">
                    <span className="telemetry-current-key">Latitude</span>
                    <span className="telemetry-current-val mono-data">
                      {formatValue(latest?.latitude)} °
                    </span>
                  </div>
                  <div className="telemetry-current-row">
                    <span className="telemetry-current-key">Longitude</span>
                    <span className="telemetry-current-val mono-data">
                      {formatValue(latest?.longitude)} °
                    </span>
                  </div>
                </div>
                {latest && (
                  <div className="telemetry-current-timestamp">
                    Last updated {formatTimestamp(latest.occurredAt)}
                  </div>
                )}
              </div>
            </div>

            {/* Zone membership badges */}
            <div className="location-tab-zones">
              <span className="section-label">Zones at Recorded Position</span>
              {activeZones.length === 0 ? (
                <span style={{ color: '#999', fontSize: 13 }}>Not inside any zone</span>
              ) : (
                <div className="location-tab-zone-badges">
                  {activeZones.map((zone) => (
                    <Tag key={zone.id} style={{ backgroundColor: zone.color, color: '#fff' }}>
                      {zone.name}
                    </Tag>
                  ))}
                </div>
              )}
            </div>
          </div>
        </>
      )}
    </div>
  );
};
