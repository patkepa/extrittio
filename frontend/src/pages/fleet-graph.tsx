import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Spinner, Callout, Icon, H4, Button } from '@blueprintjs/core';
import { useDevices } from '../hooks/use-devices';
import { useFleets } from '../hooks/use-fleets';
import { buildForceGraphData } from '../components/fleet-graph/build-force-graph-data';
import { FleetGraphCanvas } from '../components/fleet-graph/fleet-graph-canvas';
import { DevicePopover } from '../components/fleet-graph/device-popover';
import { HealthPanel } from '../components/fleet-graph/health-panel';
import type { GraphNode } from '../components/fleet-graph/build-force-graph-data';
import type { Device } from '../types/api';
import './fleet-graph.css';

interface PopoverState {
  device: Device;
  position: { x: number; y: number };
}

export const FleetGraph = () => {
  const devicesQuery = useDevices({ limit: 10000 }, { refetchInterval: 30_000 });
  const devices = devicesQuery.data?.data ?? [];
  const devicesLoading = devicesQuery.isLoading;
  const devicesError = devicesQuery.error;
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

  const prevNodesRef = useRef<GraphNode[]>();

  const graphData = useMemo(() => {
    if (devices.length === 0) return null;
    const data = buildForceGraphData(devices, fleets, prevNodesRef.current);
    // Intentional side effect: cache nodes for next merge
    prevNodesRef.current = data.nodes;
    return data;
  }, [devices, fleets]);

  const handleNodeClick = useCallback(
    (device: Device, screenPos: { x: number; y: number }) => {
      const rect = containerRef.current?.getBoundingClientRect() ?? { left: 0, top: 0, width: 0, height: 0 };
      const x = Math.max(0, Math.min(screenPos.x - rect.left, rect.width - 280));
      const y = Math.max(0, Math.min(screenPos.y - rect.top, rect.height - 300));
      setPopover({
        device,
        position: { x, y },
      });
    },
    [],
  );

  const handleBackgroundClick = useCallback(() => {
    setPopover(null);
  }, []);

  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [healthPanelOpen, setHealthPanelOpen] = useState(true);

  const handlePanelDeviceClick = useCallback(
    (nodeId: string) => {
      setSelectedNodeId(nodeId);
      // Also open popover for the device
      const node = graphData?.nodes.find((n) => n.id === nodeId);
      if (node?.type === 'device' && node.device) {
        const rect = containerRef.current?.getBoundingClientRect() ?? { left: 0, top: 0, width: 0, height: 0 };
        setPopover({
          device: node.device,
          position: { x: rect.width / 2 - 130, y: rect.height / 2 - 150 },
        });
      }
    },
    [graphData],
  );

  // Dismiss popover on Escape key
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setPopover(null);
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  // Always render the container so ResizeObserver can measure it.
  // Show loading/error/empty states inside the canvas area.
  return (
    <div className="fleet-graph-page">
      <div className="fleet-graph-canvas" ref={containerRef}>
        {error ? (
          <Callout intent="danger" icon="error">
            Failed to load fleet data. Is the backend running?
          </Callout>
        ) : isLoading ? (
          <div className="fleet-graph-empty"><Spinner /></div>
        ) : devices.length === 0 ? (
          <div className="fleet-graph-empty">
            <Icon icon="graph" size={48} />
            <H4>No devices yet</H4>
            <p>Add devices to see your fleet graph</p>
          </div>
        ) : graphData && dimensions.width > 0 ? (
          <FleetGraphCanvas
            graphData={graphData}
            width={dimensions.width}
            height={dimensions.height}
            onNodeClick={handleNodeClick}
            onBackgroundClick={handleBackgroundClick}
            selectedNodeId={selectedNodeId}
          />
        ) : null}

        {popover && (
          <DevicePopover
            device={popover.device}
            position={popover.position}
            onClose={() => setPopover(null)}
          />
        )}

        {graphData && (
          <Button
            className="health-panel-toggle"
            icon={healthPanelOpen ? 'chevron-right' : 'chevron-left'}
            minimal
            small
            title={healthPanelOpen ? 'Hide health panel' : 'Show health panel'}
            onClick={() => setHealthPanelOpen((v) => !v)}
          />
        )}
      </div>

      {graphData && healthPanelOpen && (
        <HealthPanel
          nodes={graphData.nodes}
          onDeviceClick={handlePanelDeviceClick}
          selectedNodeId={selectedNodeId}
          height={dimensions.height || 600}
        />
      )}
    </div>
  );
};
