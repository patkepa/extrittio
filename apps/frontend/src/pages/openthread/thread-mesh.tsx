import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { AxiosError } from 'axios';
import { Button, Callout, H4, Icon, Spinner, Tag } from '@blueprintjs/core';
import { FleetGraphCanvas } from '../../components/fleet-graph/fleet-graph-canvas';
import type { GraphActions } from '../../components/fleet-graph/fleet-graph-canvas';
import type { GraphNode } from '../../components/fleet-graph/build-force-graph-data';
import { useThreadMeshScan, useThreadStatus } from '../../hooks/use-thread';
import { buildThreadMeshGraphData, isCurrentThreadNetwork } from './thread-mesh-graph-data';

function apiErrorMessage(error: unknown): string {
  const axiosError = error as AxiosError<{ error?: string }>;
  return axiosError.response?.data?.error ?? 'Unable to scan the OpenThread mesh.';
}

function scanTimestamp(value: Date | null): string {
  return value ? `Last scan ${value.toLocaleTimeString()}` : 'Live radio and mesh diagnostics';
}

export function ThreadMesh() {
  const statusQuery = useThreadStatus();
  const meshScan = useThreadMeshScan();
  const { mutate: scan, isPending } = meshScan;
  const status = statusQuery.data;
  const [lastScannedAt, setLastScannedAt] = useState<Date | null>(null);
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const canvasRef = useRef<HTMLDivElement>(null);
  const graphActionsRef = useRef<GraphActions | null>(null);
  const autoScanStartedRef = useRef(false);
  const [dimensions, setDimensions] = useState({ width: 0, height: 0 });

  const runScan = useCallback(() => {
    scan(undefined, {
      onSuccess: () => setLastScannedAt(new Date()),
    });
  }, [scan]);

  useEffect(() => {
    if (!status?.connected || autoScanStartedRef.current) return;
    autoScanStartedRef.current = true;
    runScan();
  }, [runScan, status?.connected]);

  useEffect(() => {
    const element = canvasRef.current;
    if (!element) return;
    const observer = new ResizeObserver(([entry]) => {
      if (!entry) return;
      setDimensions({
        width: entry.contentRect.width,
        height: entry.contentRect.height,
      });
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  const graphData = useMemo(() => {
    if (!meshScan.data || !status) return null;
    return buildThreadMeshGraphData(meshScan.data, status);
  }, [meshScan.data, status]);

  const selectedNode = useMemo(
    () => graphData?.nodes.find((node) => node.id === selectedNodeId) ?? null,
    [graphData, selectedNodeId],
  );

  const nearbyNetworkCount = useMemo(() => {
    if (!meshScan.data || !status) return 0;
    return meshScan.data.networks.filter((network) => !isCurrentThreadNetwork(network, status))
      .length;
  }, [meshScan.data, status]);
  const clientCount =
    meshScan.data?.devices.filter((device) => !device.is_border_router).length ?? 0;
  const routerCount =
    meshScan.data?.devices.filter(
      (device) => !device.is_border_router && ['router', 'leader'].includes(device.role ?? ''),
    ).length ?? 0;

  const handleGraphNodeClick = useCallback((node: GraphNode) => {
    setSelectedNodeId(node.id);
  }, []);

  return (
    <div className="thread-mesh-view">
      <div className="thread-mesh-toolbar">
        <div className="thread-mesh-toolbar-identity">
          <Icon icon="satellite" size={16} />
          <div>
            <strong>{selectedNode?.name ?? 'OpenThread Mesh'}</strong>
            <span>{selectedNode ? 'Selected topology node' : scanTimestamp(lastScannedAt)}</span>
          </div>
        </div>

        <div className="thread-mesh-metrics" aria-label="OpenThread mesh summary">
          <div>
            <span>Active network</span>
            <strong>{status?.network_name ?? '—'}</strong>
          </div>
          <div>
            <span>Clients</span>
            <strong>{clientCount}</strong>
          </div>
          <div>
            <span>Routers</span>
            <strong>{routerCount}</strong>
          </div>
          <div>
            <span>Nearby</span>
            <strong>{nearbyNetworkCount}</strong>
          </div>
          <div>
            <span>Channel</span>
            <strong>{status?.channel ?? '—'}</strong>
          </div>
        </div>

        <div className="thread-mesh-actions">
          <Button
            icon="zoom-to-fit"
            minimal
            disabled={!graphData}
            aria-label="Fit mesh graph"
            title="Fit graph"
            onClick={() => graphActionsRef.current?.fitView()}
          />
          <Button
            icon="zoom-in"
            minimal
            disabled={!graphData}
            aria-label="Zoom in"
            onClick={() => graphActionsRef.current?.zoomIn()}
          />
          <Button
            icon="zoom-out"
            minimal
            disabled={!graphData}
            aria-label="Zoom out"
            onClick={() => graphActionsRef.current?.zoomOut()}
          />
          <Button
            icon="search"
            intent="primary"
            loading={isPending}
            disabled={!status?.connected}
            onClick={runScan}
          >
            Scan Mesh
          </Button>
        </div>
      </div>

      <div className="thread-mesh-workspace">
        <div className="thread-mesh-canvas" ref={canvasRef}>
          {statusQuery.isLoading ? (
            <MeshEmptyState loading title="Checking OpenThread runtime" />
          ) : statusQuery.isError ? (
            <div className="thread-mesh-callout">
              <Callout intent="danger" icon="error" title="OpenThread status is unavailable">
                The border-router status could not be loaded.
              </Callout>
            </div>
          ) : !status?.connected ? (
            <div className="thread-mesh-callout">
              <Callout intent="warning" icon="warning-sign" title="Border router is not connected">
                Open OpenThread Settings, connect an RCP, and refresh the runtime before scanning
                the mesh.
              </Callout>
            </div>
          ) : meshScan.isError && !graphData ? (
            <div className="thread-mesh-callout">
              <Callout intent="danger" icon="error" title="Mesh scan failed">
                {apiErrorMessage(meshScan.error)}
              </Callout>
            </div>
          ) : isPending && !graphData ? (
            <MeshEmptyState
              loading
              title="Scanning the OpenThread mesh"
              description="Discovering nearby networks and querying attached Thread nodes…"
            />
          ) : graphData && dimensions.width > 0 && dimensions.height > 0 ? (
            <FleetGraphCanvas
              graphData={graphData}
              width={dimensions.width}
              height={dimensions.height}
              onGraphNodeClick={handleGraphNodeClick}
              onBackgroundClick={() => setSelectedNodeId(null)}
              selectedNodeId={selectedNodeId}
              graphActionsRef={graphActionsRef}
              showAlertBadges={false}
              showDeviceLabels
            />
          ) : (
            <MeshEmptyState
              title="No mesh scan yet"
              description="Scan to map the active network, its clients, and nearby Thread networks."
            />
          )}

          {graphData ? (
            <div className="thread-mesh-legend" aria-label="Mesh graph legend">
              <span>
                <i className="current-network" />
                Active network
              </span>
              <span>
                <i className="border-router" />
                Border router
              </span>
              <span>
                <i className="mesh-client" />
                Mesh client
              </span>
              <span>
                <i className="nearby-network" />
                Nearby network
              </span>
            </div>
          ) : null}

          {isPending && graphData ? (
            <Tag className="thread-mesh-scanning" icon="search" intent="primary" round>
              Updating topology…
            </Tag>
          ) : null}
        </div>

        {selectedNode ? (
          <ThreadNodeDetails node={selectedNode} onClose={() => setSelectedNodeId(null)} />
        ) : null}
      </div>
    </div>
  );
}

function MeshEmptyState({
  loading = false,
  title,
  description,
}: {
  loading?: boolean;
  title: string;
  description?: string;
}) {
  return (
    <div className="thread-mesh-empty">
      {loading ? <Spinner size={32} /> : <Icon icon="graph" size={48} />}
      <H4>{title}</H4>
      {description ? <p>{description}</p> : null}
    </div>
  );
}

function ThreadNodeDetails({ node, onClose }: { node: GraphNode; onClose: () => void }) {
  return (
    <aside className="thread-node-details" aria-label={`${node.name} details`}>
      <div className="thread-node-details-header">
        <div>
          <span>Topology node</span>
          <H4>{node.name}</H4>
        </div>
        <Button icon="cross" minimal aria-label="Close node details" onClick={onClose} />
      </div>
      <div className="thread-node-details-body">
        {(node.details ?? []).map((detail) => (
          <div key={detail.label}>
            <dt>{detail.label}</dt>
            <dd>{detail.value}</dd>
          </div>
        ))}
      </div>
    </aside>
  );
}
