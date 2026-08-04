import { useState, useCallback, useEffect, useMemo, useRef, type MutableRefObject } from 'react';
import { useMap } from 'react-leaflet';
import { Button } from '@blueprintjs/core';
import { DeviceMap } from '../components/map/device-map';
import { ZoneLayer } from '../components/map/zone-layer';
import { ZonePanel } from '../components/map/zone-panel';
import type { MapDevice } from '../components/map/zone-panel';
import { ZoneDrawControls } from '../components/map/zone-draw-controls';
import { DeviceCanvasLayer } from '../components/map/device-canvas-layer';
import { useAllDevices } from '../hooks/use-devices';
import { useZones } from '../hooks/use-zones';
import type { Device } from '../types/api';
import type { Zone, CircleGeometry, PolygonGeometry } from '../types/zones';
import { useUIStore } from '../stores/ui-store';
import L from 'leaflet';
import './map-page.css';

const EMPTY_DEVICES: Device[] = [];

/** Captures the Leaflet map instance so the page can call flyToBounds. */
function MapRef({ mapRef }: { mapRef: MutableRefObject<L.Map | null> }) {
  const map = useMap();

  useEffect(() => {
    mapRef.current = map;
    return () => {
      if (mapRef.current === map) {
        mapRef.current = null;
      }
    };
  }, [map, mapRef]);

  return null;
}

type DeviceLocationFields = {
  latest_latitude?: number | null;
  latest_longitude?: number | null;
};

type LocatedDevice = Device & {
  latest_latitude: number;
  latest_longitude: number;
};

function hasLocation(device: Device): device is LocatedDevice {
  const candidate = device as Device & DeviceLocationFields;
  return (
    typeof candidate.latest_latitude === 'number' && typeof candidate.latest_longitude === 'number'
  );
}

export default function MapPage() {
  const { data: devicesData } = useAllDevices(undefined, { refetchInterval: 30_000 });
  const devices = devicesData?.data ?? EMPTY_DEVICES;
  const { data: zones = [] } = useZones();

  const [drawMode, setDrawMode] = useState(false);
  const panelOpen = useUIStore((state) => state.isContextPanelOpen);
  const toggleContextPanel = useUIStore((state) => state.toggleContextPanel);
  const [hiddenZoneIds, setHiddenZoneIds] = useState<Set<string>>(new Set());
  const mapRef = useRef<L.Map | null>(null);
  const acceptDrawnLayerRef = useRef<((layer: L.Layer, type: string) => void) | null>(null);

  const handleToggleDrawMode = useCallback(() => {
    setDrawMode((prev) => !prev);
  }, []);

  const handleDrawCreated = useCallback((layer: L.Layer, type: string) => {
    acceptDrawnLayerRef.current?.(layer, type);
  }, []);

  const handleZoneClick = useCallback((zone: Zone) => {
    const map = mapRef.current;
    if (!map) return;

    if (zone.geometry_type === 'circle') {
      const geo = zone.geometry_json as CircleGeometry;
      const center = L.latLng(geo.center[0], geo.center[1]);
      const bounds = center.toBounds(geo.radius_meters * 2);
      map.flyToBounds(bounds, { padding: [50, 50], maxZoom: 16 });
    } else {
      const geo = zone.geometry_json as PolygonGeometry;
      const bounds = L.latLngBounds(geo.points.map(([lat, lng]) => L.latLng(lat, lng)));
      map.flyToBounds(bounds, { padding: [50, 50], maxZoom: 16 });
    }
  }, []);

  const handleDeviceClick = useCallback((device: MapDevice) => {
    const map = mapRef.current;
    if (!map) return;
    map.flyTo([device.latest_latitude, device.latest_longitude], 16);
  }, []);

  const handleToggleZoneVisibility = useCallback((id: string) => {
    setHiddenZoneIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  const devicesWithLocation: MapDevice[] = useMemo(
    () =>
      devices.filter(hasLocation).map((device) => ({
        id: device.id,
        name: device.name,
        status: device.status,
        latest_latitude: device.latest_latitude,
        latest_longitude: device.latest_longitude,
        last_seen_at: device.last_seen_at,
      })),
    [devices],
  );

  const visibleZones = useMemo(
    () => zones.filter((z) => !hiddenZoneIds.has(z.id)),
    [hiddenZoneIds, zones],
  );

  return (
    <div className="map-page">
      <div className="map-page-content">
        <DeviceMap zoom={5}>
          <MapRef mapRef={mapRef} />
          <ZoneLayer zones={visibleZones} />
          <ZoneDrawControls enabled={drawMode} onCreated={handleDrawCreated} />
          <DeviceCanvasLayer devices={devicesWithLocation} />
        </DeviceMap>
      </div>
      <Button
        className="map-panel-toggle"
        icon={panelOpen ? 'chevron-right' : 'chevron-left'}
        minimal
        small
        title={panelOpen ? 'Hide panel (⇧⌘B)' : 'Show panel (⇧⌘B)'}
        onClick={toggleContextPanel}
      />
      <ZonePanel
        drawMode={drawMode}
        onToggleDrawMode={handleToggleDrawMode}
        onZoneClick={handleZoneClick}
        onDeviceClick={handleDeviceClick}
        hiddenZoneIds={hiddenZoneIds}
        onToggleZoneVisibility={handleToggleZoneVisibility}
        devices={devicesWithLocation}
        acceptDrawnLayerRef={acceptDrawnLayerRef}
        collapsed={!panelOpen}
      />
    </div>
  );
}
