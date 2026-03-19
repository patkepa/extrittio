import { useEffect, useRef, useCallback } from "react";
import { CircleMarker, Popup, useMap } from "react-leaflet";
import { Link } from "react-router-dom";
import type L from "leaflet";

interface DeviceMarkerProps {
  deviceId: string;
  deviceName: string;
  status: string;
  latitude: number;
  longitude: number;
  speed?: number | null;
  lastSeen?: string | null;
}

const STATUS_COLORS: Record<string, string> = {
  online: "#43bf4d",
  offline: "#868686",
  warning: "#d4a017",
};

function radiusForZoom(zoom: number): number {
  // Scale: zoom 3 → 3px, zoom 10 → 6px, zoom 14+ → 8px
  return Math.max(3, Math.min(8, Math.round(zoom * 0.55)));
}

export function DeviceMarker({
  deviceId, deviceName, status, latitude, longitude, speed, lastSeen,
}: DeviceMarkerProps) {
  const color = STATUS_COLORS[status] || STATUS_COLORS.offline;
  const map = useMap();
  const markerRef = useRef<L.CircleMarker>(null);

  const updateRadius = useCallback(() => {
    markerRef.current?.setRadius(radiusForZoom(map.getZoom()));
  }, [map]);

  useEffect(() => {
    updateRadius();
    map.on("zoom", updateRadius);
    return () => { map.off("zoom", updateRadius); };
  }, [map, updateRadius]);

  return (
    <CircleMarker
      ref={markerRef}
      center={[latitude, longitude]}
      radius={radiusForZoom(map.getZoom())}
      pathOptions={{ color, fillColor: color, fillOpacity: 0.8, weight: 2 }}
    >
      <Popup>
        <div style={{ color: "#e0e0e0", background: "#242424", padding: "4px 8px", borderRadius: 4, fontSize: 12 }}>
          <strong>{deviceName}</strong><br />
          Status: {status}
          {speed != null && <><br />Speed: {speed.toFixed(1)} m/s</>}
          {lastSeen && <><br />Last seen: {lastSeen}</>}
          <br />
          <Link to={`/devices/${deviceId}?tab=location`}>View details</Link>
        </div>
      </Popup>
    </CircleMarker>
  );
}
