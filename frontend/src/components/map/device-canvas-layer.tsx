import { useEffect, useRef } from 'react';
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
const HIT_RADIUS = 10;

function getStatusColor(status: string) {
  return STATUS_COLORS[status] ?? '#868686';
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
  const devicesRef = useRef(devices);
  const drawRef = useRef<(() => void) | null>(null);

  useEffect(() => {
    devicesRef.current = devices;
    drawRef.current?.();
  }, [devices]);

  useEffect(() => {
    const canvas = L.DomUtil.create('canvas', 'device-canvas-layer');
    const ctx = canvas.getContext('2d');
    const pane = map.getPanes().overlayPane;
    let frameId: number | null = null;

    pane.appendChild(canvas);

    const scheduleDraw = () => {
      if (frameId != null) return;
      frameId = window.requestAnimationFrame(() => {
        frameId = null;
        draw();
      });
    };

    const resize = () => {
      const size = map.getSize();
      const dpr = window.devicePixelRatio || 1;
      canvas.width = Math.round(size.x * dpr);
      canvas.height = Math.round(size.y * dpr);
      canvas.style.width = `${size.x}px`;
      canvas.style.height = `${size.y}px`;
      scheduleDraw();
    };

    const draw = () => {
      if (!ctx) return;

      const size = map.getSize();
      const dpr = window.devicePixelRatio || 1;
      const topLeft = map.containerPointToLayerPoint([0, 0]);
      const visibleBounds = map.getBounds().pad(0.05);

      L.DomUtil.setPosition(canvas, topLeft);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, size.x, size.y);
      ctx.lineWidth = 2;
      ctx.strokeStyle = 'rgba(255, 255, 255, 0.35)';

      for (const device of devicesRef.current) {
        const latLng = L.latLng(device.latest_latitude, device.latest_longitude);
        if (!visibleBounds.contains(latLng)) continue;

        const point = map.latLngToLayerPoint(latLng).subtract(topLeft);
        const color = getStatusColor(device.status);

        ctx.fillStyle = color;
        ctx.beginPath();
        ctx.arc(point.x, point.y, MARKER_RADIUS, 0, Math.PI * 2);
        ctx.fill();
        ctx.stroke();
      }
    };

    const findDeviceAtPoint = (point: L.Point) => {
      let closestDevice: MapDevice | null = null;
      let closestDistance = HIT_RADIUS * HIT_RADIUS;

      for (const device of devicesRef.current) {
        const devicePoint = map.latLngToContainerPoint([
          device.latest_latitude,
          device.latest_longitude,
        ]);
        const dx = devicePoint.x - point.x;
        const dy = devicePoint.y - point.y;
        const distance = dx * dx + dy * dy;

        if (distance <= closestDistance) {
          closestDistance = distance;
          closestDevice = device;
        }
      }

      return closestDevice;
    };

    const handleClick = (event: L.LeafletMouseEvent) => {
      const device = findDeviceAtPoint(event.containerPoint);
      if (!device) return;

      L.popup({ className: 'device-map-popup' })
        .setLatLng([device.latest_latitude, device.latest_longitude])
        .setContent(
          createPopupContent(device, (deviceId) => {
            navigate(`/devices/${encodeURIComponent(deviceId)}?tab=location`);
          }),
        )
        .openOn(map);
    };

    drawRef.current = draw;
    resize();
    map.on('resize zoom move viewreset zoomend moveend', scheduleDraw);
    map.on('click', handleClick);

    return () => {
      drawRef.current = null;
      if (frameId != null) {
        window.cancelAnimationFrame(frameId);
      }
      map.off('resize zoom move viewreset zoomend moveend', scheduleDraw);
      map.off('click', handleClick);
      canvas.remove();
    };
  }, [map, navigate]);

  return null;
}
