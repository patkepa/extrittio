import { useEffect, useMemo, useRef } from 'react';
import { useMap } from 'react-leaflet';
import { useNavigate } from 'react-router-dom';
import L from 'leaflet';
import type { MapDevice } from './zone-panel';

interface DeviceCanvasLayerProps {
  devices: MapDevice[];
}

const STATUS_COLORS: Record<string, string> = {
  online: '#43bf4d',
  offline: '#868686',
  warning: '#d4a017',
};

const MARKER_RADIUS = 6;

function getStatusColor(status: string) {
  return STATUS_COLORS[status] ?? '#868686';
}

function getMarkerStyle(status: string): L.CircleMarkerOptions {
  const color = getStatusColor(status);

  return {
    radius: MARKER_RADIUS,
    fillColor: color,
    fillOpacity: 1,
    color: 'rgba(255, 255, 255, 0.35)',
    opacity: 1,
    weight: 2,
  };
}

function createPopupContent(device: MapDevice, onNavigate: (deviceId: string) => void) {
  const root = L.DomUtil.create('div', 'device-map-popup-content');

  const title = document.createElement('strong');
  title.textContent = device.name;
  root.appendChild(title);
  root.appendChild(document.createElement('br'));
  root.append(`Status: ${device.status}`);

  if (device.last_seen_at) {
    root.appendChild(document.createElement('br'));
    root.append(`Last seen: ${device.last_seen_at}`);
  }

  root.appendChild(document.createElement('br'));

  const link = document.createElement('a');
  link.href = `/devices/${encodeURIComponent(device.id)}?tab=location`;
  link.textContent = 'View details';
  link.addEventListener('click', (event) => {
    event.preventDefault();
    onNavigate(device.id);
  });
  root.appendChild(link);

  return root;
}

export function DeviceCanvasLayer({ devices }: DeviceCanvasLayerProps) {
  const map = useMap();
  const navigate = useNavigate();
  const layerGroupRef = useRef<L.LayerGroup | null>(null);
  const markerLayersRef = useRef<Map<string, L.CircleMarker>>(new Map());
  const renderer = useMemo(() => L.canvas({ padding: 0.5 }), []);

  useEffect(() => {
    const layerGroup = L.layerGroup().addTo(map);
    const markerLayers = markerLayersRef.current;
    layerGroupRef.current = layerGroup;

    return () => {
      markerLayers.clear();
      layerGroup.clearLayers();
      layerGroup.remove();
      renderer.remove();
      layerGroupRef.current = null;
    };
  }, [map, renderer]);

  useEffect(() => {
    const layerGroup = layerGroupRef.current;
    if (!layerGroup) return;

    const markers = markerLayersRef.current;
    const nextDeviceIds = new Set<string>();

    for (const device of devices) {
      nextDeviceIds.add(device.id);

      const marker =
        markers.get(device.id) ??
        L.circleMarker([device.latest_latitude, device.latest_longitude], {
          ...getMarkerStyle(device.status),
          renderer,
        }).addTo(layerGroup);

      marker.setLatLng([device.latest_latitude, device.latest_longitude]);
      marker.setStyle(getMarkerStyle(device.status));
      marker.bindPopup(() =>
        createPopupContent(device, (deviceId) => {
          navigate(`/devices/${encodeURIComponent(deviceId)}?tab=location`);
        }),
      );

      markers.set(device.id, marker);
    }

    for (const [deviceId, marker] of markers) {
      if (!nextDeviceIds.has(deviceId)) {
        layerGroup.removeLayer(marker);
        markers.delete(deviceId);
      }
    }
  }, [devices, navigate, renderer]);

  return null;
}
