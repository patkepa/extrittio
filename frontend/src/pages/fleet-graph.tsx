import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Spinner, Callout, Icon, H4 } from '@blueprintjs/core';
import { useDevices, useBulkChangeFleet } from '../hooks/use-devices';
import { useFleets } from '../hooks/use-fleets';
import { useAlerts } from '../hooks/use-alerts';
import { buildForceGraphData } from '../components/fleet-graph/build-force-graph-data';
import { FleetGraphCanvas } from '../components/fleet-graph/fleet-graph-canvas';
import type {
  AlertSeverity,
  DeviceAlertBadge,
  GraphActions,
} from '../components/fleet-graph/fleet-graph-canvas';
import { HealthPanel } from '../components/fleet-graph/health-panel';
import {
  FleetGraphContextMenu,
  type ContextMenuState,
} from '../components/fleet-graph/fleet-graph-context-menu';
import { FleetGraphBulkBar } from '../components/fleet-graph/fleet-graph-bulk-bar';
import { FleetGraphBottomToolbar } from '../components/fleet-graph/fleet-graph-bottom-toolbar';
import type { FleetGraphDisplayOptions } from '../components/fleet-graph/fleet-graph-bottom-toolbar';
import { FleetGraphToolbar } from '../components/fleet-graph/fleet-graph-toolbar';
import { useSelectionStore } from '../stores/selection-store';
import { showSuccessToast, showErrorToast } from '../utils/toaster';
import type { GraphNode } from '../components/fleet-graph/build-force-graph-data';
import type { ViewportInfo } from '../components/fleet-graph/fleet-graph-minimap';
import type { Device } from '../types/api';
import './fleet-graph.css';

export const FleetGraph = () => {
  const devicesQuery = useDevices({ limit: 10000 }, { refetchInterval: 30_000 });
  const devices = useMemo(() => devicesQuery.data?.data ?? [], [devicesQuery.data?.data]);
  const devicesLoading = devicesQuery.isLoading;
  const devicesError = devicesQuery.error;
  const { data: fleetsData, isLoading: fleetsLoading, error: fleetsError } = useFleets();
  const fleets = useMemo(() => fleetsData ?? [], [fleetsData]);
  const activeAlertsQuery = useAlerts({ status: 'active', limit: 10000 });
  const [toolbarDeviceId, setToolbarDeviceId] = useState<string | null>(null);
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
  const [displayOptions, setDisplayOptions] = useState<FleetGraphDisplayOptions>({
    labels: true,
    alerts: true,
    fleets: true,
    offline: true,
  });
  const bulkFleetMutation = useBulkChangeFleet();
  const containerRef = useRef<HTMLDivElement>(null);
  const [dimensions, setDimensions] = useState({ width: 0, height: 0 });
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [hoveredNodeId, setHoveredNodeId] = useState<string | null>(null);
  const [healthPanelOpen, setHealthPanelOpen] = useState(true);
  const viewportRef = useRef<ViewportInfo | null>(null);
  const minimapDrawRef = useRef<(() => void) | null>(null);
  const graphActionsRef = useRef<GraphActions | null>(null);

  const visibleDevices = useMemo(
    () =>
      displayOptions.offline ? devices : devices.filter((device) => device.status !== 'offline'),
    [devices, displayOptions.offline],
  );

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

  // Graph data with position continuity — uses "adjusting state from props"
  // pattern to avoid reading a ref during render while preserving previous
  // node positions for smooth animation across data refreshes.
  const [graphState, setGraphState] = useState<{
    data: ReturnType<typeof buildForceGraphData> | null;
    prevNodes: GraphNode[] | undefined;
    inputDevices: typeof visibleDevices;
    inputFleets: typeof fleets;
  }>(() => {
    const data =
      visibleDevices.length === 0 ? null : buildForceGraphData(visibleDevices, fleets, undefined);
    return { data, prevNodes: data?.nodes, inputDevices: visibleDevices, inputFleets: fleets };
  });

  if (visibleDevices !== graphState.inputDevices || fleets !== graphState.inputFleets) {
    const newData =
      visibleDevices.length === 0
        ? null
        : buildForceGraphData(visibleDevices, fleets, graphState.prevNodes);
    setGraphState({
      data: newData,
      prevNodes: newData?.nodes ?? graphState.prevNodes,
      inputDevices: visibleDevices,
      inputFleets: fleets,
    });
  }

  const graphData = useMemo(() => {
    if (!graphState.data) return null;
    if (displayOptions.fleets) return graphState.data;

    return {
      nodes: graphState.data.nodes
        .filter((node) => node.type === 'device')
        .map((node) => ({ ...node, neighbors: [], links: [] })),
      links: [],
    };
  }, [displayOptions.fleets, graphState.data]);

  const sidebarGraphData = graphState.data;

  const toolbarDevice = useMemo(
    () => devices.find((device) => device.id === toolbarDeviceId) ?? null,
    [devices, toolbarDeviceId],
  );

  const alertBadges = useMemo<Record<string, DeviceAlertBadge>>(() => {
    const severityRank: Record<AlertSeverity, number> = { info: 0, warning: 1, critical: 2 };
    const badges: Record<string, DeviceAlertBadge> = {};

    for (const alert of activeAlertsQuery.data?.data ?? []) {
      const severity = alert.severity as AlertSeverity;
      const existing = badges[alert.device_id];
      if (!existing) {
        badges[alert.device_id] = { count: 1, severity };
        continue;
      }

      existing.count += 1;
      if (severityRank[severity] > severityRank[existing.severity]) {
        existing.severity = severity;
      }
    }

    return badges;
  }, [activeAlertsQuery.data?.data]);

  const handleNodeClick = useCallback((device: Device) => {
    setToolbarDeviceId(device.id);
    setSelectedNodeId(`device-${device.id}`);
  }, []);

  const handleBackgroundClick = useCallback(() => {
    setToolbarDeviceId(null);
    setSelectedNodeId(null);
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
      setToolbarDeviceId(node.device.id);
      setSelectedNodeId(node.id);
    }
  }, []);

  const handleAssignFleet = useCallback(
    async (deviceIds: string[], fleetId: number) => {
      const rawIds = deviceIds.map((id) => id.replace(/^device-/, ''));
      try {
        const result = await bulkFleetMutation.mutateAsync({
          device_ids: rawIds,
          fleet_id: fleetId,
        });
        void showSuccessToast(
          `${result.affected} device${result.affected !== 1 ? 's' : ''} moved to fleet`,
        );
      } catch {
        void showErrorToast('Failed to change fleet');
      }
    },
    [bulkFleetMutation],
  );

  const handleRemoveFromFleet = useCallback(
    async (deviceIds: string[]) => {
      const rawIds = deviceIds.map((id) => id.replace(/^device-/, ''));
      try {
        const result = await bulkFleetMutation.mutateAsync({ device_ids: rawIds, fleet_id: null });
        void showSuccessToast(
          `${result.affected} device${result.affected !== 1 ? 's' : ''} removed from fleet`,
        );
      } catch {
        void showErrorToast('Failed to remove from fleet');
      }
    },
    [bulkFleetMutation],
  );

  const handleViewportChange = useCallback((t: ViewportInfo) => {
    viewportRef.current = t;
    minimapDrawRef.current?.();
  }, []);

  const handleFrameRedraw = useCallback(() => {
    minimapDrawRef.current?.();
  }, []);

  const handlePanelDeviceClick = useCallback(
    (nodeId: string) => {
      setSelectedNodeId(nodeId);
      const node = sidebarGraphData?.nodes.find((n) => n.id === nodeId);
      if (node?.type === 'device' && node.device) {
        setToolbarDeviceId(node.device.id);
      }
    },
    [sidebarGraphData],
  );

  const handleClearToolbarDevice = useCallback(() => {
    setToolbarDeviceId(null);
    setSelectedNodeId(null);
  }, []);

  useEffect(() => {
    const handler = () => setHealthPanelOpen((v) => !v);
    window.addEventListener('toggle-right-sidebar', handler);
    return () => window.removeEventListener('toggle-right-sidebar', handler);
  }, []);

  // Prune stale selections on data refresh
  useEffect(() => {
    if (!sidebarGraphData) return;
    const currentDeviceIds = new Set(
      sidebarGraphData.nodes.filter((n) => n.type === 'device').map((n) => n.id),
    );
    const { selectedDeviceIds, removeFromSelection } = useSelectionStore.getState();
    const staleIds = Array.from(selectedDeviceIds).filter((id) => !currentDeviceIds.has(id));
    if (staleIds.length > 0) {
      removeFromSelection(staleIds);
    }
  }, [sidebarGraphData]);

  // Clear selection when navigating away from fleet graph page
  useEffect(() => {
    return () => {
      useSelectionStore.getState().clearSelection();
    };
  }, []);

  // Dismiss context menu → toolbar details → selection on Escape key (priority ordering)
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        if (contextMenu) {
          setContextMenu(null);
        } else if (toolbarDeviceId) {
          handleClearToolbarDevice();
        } else {
          useSelectionStore.getState().clearSelection();
        }
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [contextMenu, handleClearToolbarDevice, toolbarDeviceId]);

  // Always render the container so ResizeObserver can measure it.
  // Show loading/error/empty states inside the canvas area.
  return (
    <div className="fleet-graph-page">
      <div className="fleet-graph-workspace">
        <FleetGraphToolbar
          selectedDevice={toolbarDevice}
          deviceCount={visibleDevices.length}
          fleetCount={fleets.length}
          healthPanelOpen={healthPanelOpen}
          hasGraphData={Boolean(graphData)}
          onClearDevice={handleClearToolbarDevice}
          onFitView={() => graphActionsRef.current?.fitView()}
          onZoomIn={() => graphActionsRef.current?.zoomIn()}
          onZoomOut={() => graphActionsRef.current?.zoomOut()}
          onToggleHealthPanel={() => setHealthPanelOpen((v) => !v)}
        />

        <div className="fleet-graph-canvas" ref={containerRef}>
          {error ? (
            <Callout intent="danger" icon="error">
              Failed to load fleet data. Is the backend running?
            </Callout>
          ) : isLoading ? (
            <div className="fleet-graph-empty">
              <Spinner />
            </div>
          ) : devices.length === 0 ? (
            <div className="fleet-graph-empty">
              <Icon icon="graph" size={48} />
              <H4>No devices yet</H4>
              <p>Add devices to see your fleet graph</p>
            </div>
          ) : visibleDevices.length === 0 ? (
            <div className="fleet-graph-empty">
              <Icon icon="offline" size={48} />
              <H4>No visible devices</H4>
              <p>Show offline devices to bring them back into the graph</p>
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
              hoveredNodeId={hoveredNodeId}
              onViewportChange={handleViewportChange}
              graphActionsRef={graphActionsRef}
              onFrameRedraw={handleFrameRedraw}
              showDeviceLabels={displayOptions.labels}
              showAlertBadges={displayOptions.alerts}
              alertBadges={alertBadges}
            />
          ) : null}

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
        </div>

        <FleetGraphBottomToolbar
          deviceCount={visibleDevices.length}
          hasGraphData={devices.length > 0}
          visibleDevices={visibleDevices}
          displayOptions={displayOptions}
          onDisplayOptionsChange={setDisplayOptions}
        />
      </div>

      {sidebarGraphData && (
        <HealthPanel
          nodes={sidebarGraphData.nodes}
          links={sidebarGraphData.links}
          onDeviceClick={handlePanelDeviceClick}
          onDeviceHover={setHoveredNodeId}
          selectedNodeId={selectedNodeId}
          hoveredNodeId={hoveredNodeId}
          height={dimensions.height || 600}
          viewportRef={viewportRef}
          minimapDrawRef={minimapDrawRef}
          canvasWidth={dimensions.width}
          canvasHeight={dimensions.height}
          collapsed={!healthPanelOpen}
        />
      )}
    </div>
  );
};
