import { useEffect } from 'react';
import { MapContainer, TileLayer, useMap } from 'react-leaflet';
import 'leaflet/dist/leaflet.css';
import './device-map.css';

interface DeviceMapProps {
  center?: [number, number];
  zoom?: number;
  children?: React.ReactNode;
  className?: string;
}

function MapSizeObserver() {
  const map = useMap();

  useEffect(() => {
    const container = map.getContainer();
    let frameId: number | null = null;

    const invalidate = () => {
      if (frameId != null) {
        cancelAnimationFrame(frameId);
      }

      frameId = requestAnimationFrame(() => {
        map.invalidateSize({ pan: false });
        frameId = null;
      });
    };

    const resizeObserver = new ResizeObserver(invalidate);
    resizeObserver.observe(container);
    container.addEventListener('transitionend', invalidate);
    invalidate();

    return () => {
      if (frameId != null) {
        cancelAnimationFrame(frameId);
      }
      container.removeEventListener('transitionend', invalidate);
      resizeObserver.disconnect();
    };
  }, [map]);

  return null;
}

export function DeviceMap({
  center = [52.2297, 21.0122],
  zoom = 13,
  children,
  className = '',
}: DeviceMapProps) {
  return (
    <MapContainer
      center={center}
      zoom={zoom}
      className={`device-map ${className}`}
      scrollWheelZoom={true}
      preferCanvas={true}
    >
      <TileLayer
        attribution='&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a>'
        url="https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png"
      />
      <MapSizeObserver />
      {children}
    </MapContainer>
  );
}
