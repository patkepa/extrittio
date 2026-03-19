import { useState, useCallback } from "react";
import { DeviceMap } from "../components/map/device-map";
import { ZoneLayer } from "../components/map/zone-layer";
import { ZonePanel } from "../components/map/zone-panel";
import { ZoneDrawControls } from "../components/map/zone-draw-controls";
import { DeviceMarker } from "../components/map/device-marker";
import { useDevices } from "../hooks/use-devices";
import { useZones } from "../hooks/use-zones";
import L from "leaflet";
import "./map-page.css";

export default function MapPage() {
  const { data: devicesData } = useDevices(undefined, { refetchInterval: 30_000 });
  const devices = Array.isArray(devicesData) ? devicesData : (devicesData as any)?.data ?? [];
  const { data: zones = [] } = useZones();

  const [drawMode, setDrawMode] = useState(false);

  const handleToggleDrawMode = useCallback(() => {
    setDrawMode((prev) => !prev);
  }, []);

  /**
   * Called by ZoneDrawControls when the user finishes drawing a shape.
   * Routes the event into ZonePanel which handles geometry extraction
   * and the "name your zone" dialog.
   */
  const handleDrawCreated = useCallback((layer: L.Layer, type: string) => {
    ZonePanel.acceptDrawnLayer(layer, type);
  }, []);

  const devicesWithLocation = (devices as any[]).filter(
    (d: any) => d.latest_latitude != null && d.latest_longitude != null
  );

  return (
    <div className="map-page">
      <div className="map-page-content">
        <DeviceMap zoom={5}>
          <ZoneLayer zones={zones} />
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
      <ZonePanel
        drawMode={drawMode}
        onToggleDrawMode={handleToggleDrawMode}
        onDrawCreated={handleDrawCreated}
      />
    </div>
  );
}
