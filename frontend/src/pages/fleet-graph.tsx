import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Spinner, Callout, Icon, H4, Button } from '@blueprintjs/core';
import { useDevices, useBulkChangeFleet } from '../hooks/use-devices';
import { useFleets } from '../hooks/use-fleets';
import { buildForceGraphData } from '../components/fleet-graph/build-force-graph-data';
import { FleetGraphCanvas } from '../components/fleet-graph/fleet-graph-canvas';
import type { GraphActions } from '../components/fleet-graph/fleet-graph-canvas';
import { DevicePopover } from '../components/fleet-graph/device-popover';
import { HealthPanel } from '../components/fleet-graph/health-panel';
import { FleetGraphContextMenu, type ContextMenuState } from '../components/fleet-graph/fleet-graph-context-menu';
import { FleetGraphBulkBar } from '../components/fleet-graph/fleet-graph-bulk-bar';
import { useSelectionStore } from '../stores/selection-store';
import { showSuccessToast, showErrorToast } from '../utils/toaster';
import type { GraphNode } from '../components/fleet-graph/build-force-graph-data';
import type { ViewportInfo } from '../components/fleet-graph/fleet-graph-minimap';
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
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
  const bulkFleetMutation = useBulkChangeFleet();
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
    return buildForceGraphData(devices, fleets, prevNodesRef.current);
  }, [devices, fleets]);

  // Cache nodes for the next merge — kept outside useMemo to avoid
  // side effects (React Strict Mode double-invokes useMemo in dev).
  useEffect(() => {
    if (graphData) {
      prevNodesRef.current = graphData.nodes;
    }
  }, [graphData]);

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

  const handleNodeRightClick = useCallback((node: GraphNode, event: MouseEvent) => {
    event.preventDefault();
    setContextMenu({
      position: { x: event.clientX, y: event.clientY },
      target: { type: node.type, node },
    });
  }, []);

  const handleContextMenuViewDetails = useCallback((node: GraphNode) => {
    if (node.type === 'device' && node.device) {
      const rect = containerRef.current?.getBoundingClientRect() ?? { left: 0, top: 0, width: 0, height: 0 };
      setPopover({
        device: node.device,
        position: { x: rect.width / 2 - 130, y: rect.height / 2 - 150 },
      });
    }
  }, []);

  const handleAssignFleet = useCallback(async (deviceIds: string[], fleetId: number) => {
    const rawIds = deviceIds.map((id) => id.replace(/^device-/, ''));
    try {
      const result = await bulkFleetMutation.mutateAsync({ device_ids: rawIds, fleet_id: fleetId });
      void showSuccessToast(`${result.affected} device${result.affected !== 1 ? 's' : ''} moved to fleet`);
    } catch {
      void showErrorToast('Failed to change fleet');
    }
  }, [bulkFleetMutation]);

  const handleRemoveFromFleet = useCallback(async (deviceIds: string[]) => {
    const rawIds = deviceIds.map((id) => id.replace(/^device-/, ''));
    try {
      const result = await bulkFleetMutation.mutateAsync({ device_ids: rawIds, fleet_id: null });
      void showSuccessToast(`${result.affected} device${result.affected !== 1 ? 's' : ''} removed from fleet`);
    } catch {
      void showErrorToast('Failed to remove from fleet');
    }
  }, [bulkFleetMutation]);

  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [healthPanelOpen, setHealthPanelOpen] = useState(true);
  const viewportRef = useRef<ViewportInfo | null>(null);
  const minimapDrawRef = useRef<(() => void) | null>(null);
  const graphActionsRef = useRef<GraphActions | null>(null);

  const handleViewportChange = useCallback((t: ViewportInfo) => {
    viewportRef.current = t;
    minimapDrawRef.current?.();
  }, []);

  const handleMinimapNavigate = useCallback((worldX: number, worldY: number) => {
    graphActionsRef.current?.navigateTo(worldX, worldY);
  }, []);

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

  // Prune stale selections on data refresh
  useEffect(() => {
    if (!graphData) return;
    const currentDeviceIds = new Set(
      graphData.nodes.filter((n) => n.type === 'device').map((n) => n.id),
    );
    const { selectedDeviceIds, removeFromSelection } = useSelectionStore.getState();
    const staleIds = Array.from(selectedDeviceIds).filter((id) => !currentDeviceIds.has(id));
    if (staleIds.length > 0) {
      removeFromSelection(staleIds);
    }
  }, [graphData]);

  // Clear selection when navigating away from fleet graph page
  useEffect(() => {
    return () => {
      useSelectionStore.getState().clearSelection();
    };
  }, []);

  // Dismiss context menu → popover → selection on Escape key (priority ordering)
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        if (contextMenu) {
          setContextMenu(null);
        } else if (popover) {
          setPopover(null);
        } else {
          useSelectionStore.getState().clearSelection();
        }
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [contextMenu, popover]);

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
            onNodeRightClick={handleNodeRightClick}
            selectedNodeId={selectedNodeId}
            onViewportChange={handleViewportChange}
            graphActionsRef={graphActionsRef}
          />
        ) : null}

        {popover && (
          <DevicePopover
            device={popover.device}
            position={popover.position}
            onClose={() => setPopover(null)}
          />
        )}

        {contextMenu && (
          <FleetGraphContextMenu
            state={contextMenu}
            onClose={() => setContextMenu(null)}
            onViewDetails={handleContextMenuViewDetails}
            onAssignFleet={(ids, fid) => void handleAssignFleet(ids, fid)}
            onRemoveFromFleet={(ids) => void handleRemoveFromFleet(ids)}
          />
        )}

        <FleetGraphBulkBar />

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
          viewportRef={viewportRef}
          minimapDrawRef={minimapDrawRef}
          canvasWidth={dimensions.width}
          canvasHeight={dimensions.height}
          onMinimapNavigate={handleMinimapNavigate}
        />
      )}
    </div>
  );
};
