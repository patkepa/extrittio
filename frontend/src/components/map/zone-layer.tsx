import { Circle, Polygon, Tooltip } from 'react-leaflet';
import type { Zone, CircleGeometry, PolygonGeometry } from '../../types/zones';

interface ZoneLayerProps {
  zones: Zone[];
  onZoneClick?: (zone: Zone) => void;
}

export function ZoneLayer({ zones, onZoneClick }: ZoneLayerProps) {
  return (
    <>
      {zones.map((zone) => {
        const geo = zone.geometry_json;
        if (zone.geometry_type === 'circle') {
          const circleGeo = geo as CircleGeometry;
          return (
            <Circle
              key={zone.id}
              center={circleGeo.center}
              radius={circleGeo.radius_meters}
              pathOptions={{
                color: zone.color,
                fillColor: zone.color,
                fillOpacity: 0.15,
                weight: 2,
              }}
              eventHandlers={{ click: () => onZoneClick?.(zone) }}
            >
              <Tooltip permanent>{zone.name}</Tooltip>
            </Circle>
          );
        }
        const polyGeo = geo as PolygonGeometry;
        return (
          <Polygon
            key={zone.id}
            positions={polyGeo.points.map(([lat, lon]) => [lat, lon] as [number, number])}
            pathOptions={{
              color: zone.color,
              fillColor: zone.color,
              fillOpacity: 0.15,
              weight: 2,
            }}
            eventHandlers={{ click: () => onZoneClick?.(zone) }}
          >
            <Tooltip permanent>{zone.name}</Tooltip>
          </Polygon>
        );
      })}
    </>
  );
}
