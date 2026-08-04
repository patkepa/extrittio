import { Polyline, CircleMarker } from 'react-leaflet';

interface LocationTrailProps {
  points: { latitude: number; longitude: number; timestamp: string }[];
}

export function LocationTrail({ points }: LocationTrailProps) {
  if (points.length === 0) return null;
  const positions = points.map((p) => [p.latitude, p.longitude] as [number, number]);
  const lastPosition = positions[positions.length - 1] as [number, number];
  return (
    <>
      <Polyline
        positions={positions}
        pathOptions={{ color: '#5c7cfa', weight: 3, opacity: 0.7, dashArray: '5, 10' }}
      />
      <CircleMarker
        center={lastPosition}
        radius={6}
        pathOptions={{ color: '#5c7cfa', fillColor: '#5c7cfa', fillOpacity: 1, weight: 2 }}
      />
    </>
  );
}
