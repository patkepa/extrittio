import { CircleMarker, Popup } from "react-leaflet";
import { Link } from "react-router-dom";

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

export function DeviceMarker({
  deviceId, deviceName, status, latitude, longitude, speed, lastSeen,
}: DeviceMarkerProps) {
  const color = STATUS_COLORS[status] || STATUS_COLORS.offline;
  return (
    <CircleMarker
      center={[latitude, longitude]}
      radius={8}
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
