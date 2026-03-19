import { MapContainer, TileLayer } from "react-leaflet";
import "leaflet/dist/leaflet.css";
import "./device-map.css";

interface DeviceMapProps {
  center?: [number, number];
  zoom?: number;
  children?: React.ReactNode;
  className?: string;
}

export function DeviceMap({
  center = [52.2297, 21.0122],
  zoom = 13,
  children,
  className = "",
}: DeviceMapProps) {
  return (
    <MapContainer
      center={center}
      zoom={zoom}
      className={`device-map ${className}`}
      scrollWheelZoom={true}
    >
      <TileLayer
        attribution='&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a>'
        url="https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png"
      />
      {children}
    </MapContainer>
  );
}
