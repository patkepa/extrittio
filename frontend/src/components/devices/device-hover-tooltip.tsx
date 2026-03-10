import { useRef, useState, useEffect, useCallback } from 'react';
import { createPortal } from 'react-dom';
import { DeviceSummaryCard } from './device-summary-card';
import type { Device } from '../../types/api';

export function useDeviceHoverTooltip() {
  const [hoveredDevice, setHoveredDevice] = useState<Device | null>(null);
  const [hoverPos, setHoverPos] = useState({ x: 0, y: 0 });
  const hoverTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (hoverTimeoutRef.current) clearTimeout(hoverTimeoutRef.current);
    };
  }, []);

  const onMouseEnter = useCallback((device: Device, e: React.MouseEvent) => {
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    setHoverPos({ x: rect.left - 8, y: rect.top + rect.height / 2 });
    hoverTimeoutRef.current = setTimeout(() => setHoveredDevice(device), 300);
  }, []);

  const onMouseLeave = useCallback(() => {
    if (hoverTimeoutRef.current) {
      clearTimeout(hoverTimeoutRef.current);
      hoverTimeoutRef.current = null;
    }
    setHoveredDevice(null);
  }, []);

  return { hoveredDevice, hoverPos, onMouseEnter, onMouseLeave };
}

export function DeviceHoverTooltip({
  device,
  position,
}: {
  device: Device | null;
  position: { x: number; y: number };
}) {
  if (!device) return null;

  return createPortal(
    <div
      className="device-hover-tooltip"
      style={{
        position: 'fixed',
        left: position.x,
        top: position.y,
        transform: 'translate(-100%, -50%)',
        zIndex: 30,
      }}
    >
      <DeviceSummaryCard device={device} />
    </div>,
    document.body
  );
}
