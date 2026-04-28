import { useEffect } from 'react';
import { useMap } from 'react-leaflet';
import L from 'leaflet';
import 'leaflet-draw';
import 'leaflet-draw/dist/leaflet.draw.css';

interface ZoneDrawControlsProps {
  enabled: boolean;
  onCreated: (layer: L.Layer, type: string) => void;
}

type CreatedEvent = L.LeafletEvent & {
  layer: L.Layer;
  layerType: string;
};

type DrawToolbarHandle = {
  disable?: () => void;
};

type DrawControlWithToolbars = L.Control.Draw & {
  _toolbars?: Record<string, DrawToolbarHandle | undefined>;
};

function disableDrawToolbars(drawControl: DrawControlWithToolbars) {
  Object.values(drawControl._toolbars ?? {}).forEach((toolbar) => toolbar?.disable?.());
}

function restoreMapInteractions(map: L.Map) {
  map.dragging.enable();
  map.touchZoom.enable();
  map.doubleClickZoom.enable();
  map.scrollWheelZoom.enable();
  map.boxZoom.enable();
  map.keyboard.enable();
}

export function ZoneDrawControls({ enabled, onCreated }: ZoneDrawControlsProps) {
  const map = useMap();

  useEffect(() => {
    if (!enabled) return;

    const drawnItems = new L.FeatureGroup();
    map.addLayer(drawnItems);

    const drawControl = new L.Control.Draw({
      draw: {
        polyline: false,
        rectangle: false,
        marker: false,
        circlemarker: false,
        circle: { shapeOptions: { color: '#4A90D9', fillOpacity: 0.15 } },
        polygon: { shapeOptions: { color: '#4A90D9', fillOpacity: 0.15 } },
      },
      edit: { featureGroup: drawnItems },
    }) as DrawControlWithToolbars;

    map.addControl(drawControl);

    const handleCreated: L.LeafletEventHandlerFn = (event) => {
      const e = event as CreatedEvent;
      drawnItems.addLayer(e.layer);
      onCreated(e.layer, e.layerType);
      setTimeout(() => drawnItems.removeLayer(e.layer), 100);
    };

    map.on('draw:created', handleCreated);

    return () => {
      disableDrawToolbars(drawControl);
      map.removeControl(drawControl);
      map.off('draw:created', handleCreated);
      map.removeLayer(drawnItems);
      restoreMapInteractions(map);
    };
  }, [enabled, map, onCreated]);

  return null;
}
