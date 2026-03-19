import { useEffect } from "react";
import { useMap } from "react-leaflet";
import L from "leaflet";
import "leaflet-draw";
import "leaflet-draw/dist/leaflet.draw.css";

interface ZoneDrawControlsProps {
  enabled: boolean;
  onCreated: (layer: L.Layer, type: string) => void;
}

export function ZoneDrawControls({ enabled, onCreated }: ZoneDrawControlsProps) {
  const map = useMap();

  useEffect(() => {
    if (!enabled) return;

    const drawnItems = new L.FeatureGroup();
    map.addLayer(drawnItems);

    const drawControl = new (L.Control as any).Draw({
      draw: {
        polyline: false,
        rectangle: false,
        marker: false,
        circlemarker: false,
        circle: { shapeOptions: { color: "#4A90D9", fillOpacity: 0.15 } },
        polygon: { shapeOptions: { color: "#4A90D9", fillOpacity: 0.15 } },
      },
      edit: { featureGroup: drawnItems },
    });

    map.addControl(drawControl);

    const handleCreated = (e: any) => {
      drawnItems.addLayer(e.layer);
      onCreated(e.layer, e.layerType);
      setTimeout(() => drawnItems.removeLayer(e.layer), 100);
    };

    map.on("draw:created", handleCreated);

    return () => {
      map.removeControl(drawControl);
      map.removeLayer(drawnItems);
      map.off("draw:created", handleCreated);
    };
  }, [enabled, map, onCreated]);

  return null;
}
