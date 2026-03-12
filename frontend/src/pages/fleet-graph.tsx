import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Spinner, Callout, Icon, H4 } from '@blueprintjs/core';
import { useDevices } from '../hooks/use-devices';
import { useFleets } from '../hooks/use-fleets';
import { buildForceGraphData } from '../components/fleet-graph/build-force-graph-data';
import { FleetGraphCanvas } from '../components/fleet-graph/fleet-graph-canvas';
import { DevicePopover } from '../components/fleet-graph/device-popover';
import type { Device } from '../types/api';
import './fleet-graph.css';

interface PopoverState {
  device: Device;
  position: { x: number; y: number };
}

export const FleetGraph = () => {
  const { data: devices = [], isLoading: devicesLoading, error: devicesError } = useDevices({ limit: 10000 });
  const { data: fleets = [], isLoading: fleetsLoading, error: fleetsError } = useFleets();
  const [popover, setPopover] = useState<PopoverState | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [dimensions, setDimensions] = useState({ width: 0, height: 0 });

  const isLoading = devicesLoading || fleetsLoading;
  const error = devicesError || fleetsError;

  // Measure container dimensions
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;

    const observer = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (entry) {
        setDimensions({
          width: entry.contentRect.width,
          height: entry.contentRect.height,
        });
      }
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  const graphData = useMemo(() => {
    if (devices.length === 0) return null;
    return buildForceGraphData(devices, fleets);
  }, [devices, fleets]);

  const handleNodeClick = useCallback(
    (device: Device, screenPos: { x: number; y: number }) => {
      const rect = containerRef.current?.getBoundingClientRect() ?? { left: 0, top: 0 };
      setPopover({
        device,
        position: { x: screenPos.x - rect.left, y: screenPos.y - rect.top },
      });
    },
    [],
  );

  const handleBackgroundClick = useCallback(() => {
    setPopover(null);
  }, []);

  // Dismiss popover on Escape key
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setPopover(null);
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  if (error) {
    return (
      <div className="fleet-graph-page">
        <Callout intent="danger" icon="error">
          Failed to load fleet data. Is the backend running?
        </Callout>
      </div>
    );
  }

  if (isLoading) {
    return (
      <div className="fleet-graph-page">
        <Spinner />
      </div>
    );
  }

  if (devices.length === 0) {
    return (
      <div className="fleet-graph-page">
        <div className="fleet-graph-empty">
          <Icon icon="graph" size={48} />
          <H4>No devices yet</H4>
          <p>Add devices to see your fleet graph</p>
        </div>
      </div>
    );
  }

  return (
    <div className="fleet-graph-page">
      <div className="fleet-graph-canvas" ref={containerRef}>
        {graphData && dimensions.width > 0 && (
          <FleetGraphCanvas
            graphData={graphData}
            width={dimensions.width}
            height={dimensions.height}
            onNodeClick={handleNodeClick}
            onBackgroundClick={handleBackgroundClick}
          />
        )}

        {popover && (
          <DevicePopover
            device={popover.device}
            position={popover.position}
            onClose={() => setPopover(null)}
          />
        )}
      </div>
    </div>
  );
};
