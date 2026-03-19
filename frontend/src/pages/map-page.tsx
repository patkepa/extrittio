import { useState, useCallback, useRef } from "react";
import { useMap } from "react-leaflet";
import { Button } from "@blueprintjs/core";
import { DeviceMap } from "../components/map/device-map";
import { ZoneLayer } from "../components/map/zone-layer";
import { ZonePanel } from "../components/map/zone-panel";
import { ZoneDrawControls } from "../components/map/zone-draw-controls";
import { DeviceMarker } from "../components/map/device-marker";
import { useDevices } from "../hooks/use-devices";
import { useZones } from "../hooks/use-zones";
import type { Zone, CircleGeometry, PolygonGeometry } from "../types/zones";
import L from "leaflet";
import "./map-page.css";

/** Captures the Leaflet map instance so the page can call flyToBounds. */
function MapRef({ mapRef }: { mapRef: React.MutableRefObject<L.Map | null> }) {
  const map = useMap();
  mapRef.current = map;
  return null;
}

export default function MapPage() {
  const { data: devicesData } = useDevices(undefined, { refetchInterval: 30_000 });
  const devices = Array.isArray(devicesData) ? devicesData : (devicesData as any)?.data ?? [];
  const { data: zones = [] } = useZones();

  const [drawMode, setDrawMode] = useState(false);
  const [panelOpen, setPanelOpen] = useState(true);
  const [hiddenZoneIds, setHiddenZoneIds] = useState<Set<string>>(new Set());
  const mapRef = useRef<L.Map | null>(null);

  const handleToggleDrawMode = useCallback(() => {
    setDrawMode((prev) => !prev);
  }, []);

  const handleDrawCreated = useCallback((layer: L.Layer, type: string) => {
    ZonePanel.acceptDrawnLayer(layer, type);
  }, []);

  const handleZoneClick = useCallback((zone: Zone) => {
    const map = mapRef.current;
    if (!map) return;

    if (zone.geometry_type === "circle") {
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

  const handleToggleZoneVisibility = useCallback((id: string) => {
    setHiddenZoneIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  const devicesWithLocation = (devices as any[]).filter(
    (d: any) => d.latest_latitude != null && d.latest_longitude != null
  );

  const visibleZones = zones.filter((z) => !hiddenZoneIds.has(z.id));

  return (
    <div className="map-page">
      <div className="map-page-content">
        <DeviceMap zoom={5}>
          <MapRef mapRef={mapRef} />
          <ZoneLayer zones={visibleZones} />
          <ZoneDrawControls
            enabled={drawMode}
            onCreated={handleDrawCreated}
          />
          {devicesWithLocation.map((device: any) => (
            <DeviceMarker
              key={device.id}
              deviceId={device.id}
              deviceName={device.name}
              status={device.status}
              latitude={device.latest_latitude}
              longitude={device.latest_longitude}
              lastSeen={device.last_seen_at}
            />
          ))}
        </DeviceMap>
      </div>
      <Button
        className="zone-panel-toggle"
        icon={panelOpen ? "chevron-right" : "chevron-left"}
        minimal
        small
        title={panelOpen ? "Hide zone panel" : "Show zone panel"}
        onClick={() => setPanelOpen((v) => !v)}
      />
      <ZonePanel
        drawMode={drawMode}
        onToggleDrawMode={handleToggleDrawMode}
        onZoneClick={handleZoneClick}
        hiddenZoneIds={hiddenZoneIds}
        onToggleZoneVisibility={handleToggleZoneVisibility}
        collapsed={!panelOpen}
      />
    </div>
  );
}
