import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  ReactFlow,
  Controls,
  MiniMap,
  Background,
  BackgroundVariant,
  useNodesState,
  useEdgesState,
  useReactFlow,
  ReactFlowProvider,
  type NodeMouseHandler,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import { Spinner, Callout, Icon, H4 } from '@blueprintjs/core';
import { useDevices } from '../hooks/use-devices';
import { useFleets } from '../hooks/use-fleets';
import { buildFleetGraph } from '../components/fleet-graph/build-fleet-graph';
import { FleetNode } from '../components/fleet-graph/fleet-node';
import { DeviceNode } from '../components/fleet-graph/device-node';
import { DevicePopover } from '../components/fleet-graph/device-popover';
import type { Device } from '../types/api';
import './fleet-graph.css';

const nodeTypes = {
  fleetNode: FleetNode,
  deviceNode: DeviceNode,
};

interface PopoverState {
  device: Device;
  position: { x: number; y: number };
}

function FleetGraphInner() {
  const { data: devices = [], isLoading: devicesLoading, error: devicesError } = useDevices({ limit: 10000 });
  const { data: fleets = [], isLoading: fleetsLoading, error: fleetsError } = useFleets();
  const [popover, setPopover] = useState<PopoverState | null>(null);
  const canvasRef = useRef<HTMLDivElement>(null);
  const { flowToScreenPosition } = useReactFlow();

  const isLoading = devicesLoading || fleetsLoading;
  const error = devicesError || fleetsError;

  const graphData = useMemo(() => {
    if (devices.length === 0 && fleets.length === 0) return null;
    return buildFleetGraph(devices, fleets);
  }, [devices, fleets]);

  const [nodes, setNodes, onNodesChange] = useNodesState(graphData?.nodes ?? []);
  const [edges, setEdges, onEdgesChange] = useEdgesState(graphData?.edges ?? []);

  // Update nodes/edges when data changes
  useEffect(() => {
    if (graphData) {
      setNodes(graphData.nodes);
      setEdges(graphData.edges);
    }
  }, [graphData, setNodes, setEdges]);

  const onNodeClick: NodeMouseHandler = useCallback(
    (_event, node) => {
      if (node.type === 'deviceNode') {
        const device = (node.data as any).device as Device;
        const screenPos = flowToScreenPosition({
          x: node.position.x + 220,
          y: node.position.y,
        });
        // Convert viewport coords to canvas-relative coords for absolute positioning
        const rect = canvasRef.current?.getBoundingClientRect() ?? { left: 0, top: 0 };
        setPopover({
          device,
          position: { x: screenPos.x - rect.left, y: screenPos.y - rect.top },
        });
      }
    },
    [flowToScreenPosition],
  );

  const onPaneClick = useCallback(() => {
    setPopover(null);
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
      <div className="fleet-graph-canvas" ref={canvasRef}>
        <ReactFlow
          nodes={nodes}
          edges={edges}
          onNodesChange={onNodesChange}
          onEdgesChange={onEdgesChange}
          nodeTypes={nodeTypes}
          onNodeClick={onNodeClick}
          onPaneClick={onPaneClick}
          fitView
          fitViewOptions={{ padding: 0.3 }}
          nodesDraggable={false}
          nodesConnectable={false}
          elementsSelectable={false}
          proOptions={{ hideAttribution: true }}
        >
          <Background variant={BackgroundVariant.Dots} gap={24} size={1} color="rgba(255,255,255,0.05)" />
          <Controls showInteractive={false} />
          <MiniMap
            nodeColor={(node) => {
              if (node.type === 'fleetNode') return 'hsl(211, 100%, 50%)';
              const device = (node.data as any)?.device as Device | undefined;
              if (device?.status === 'online') return 'hsl(152, 69%, 45%)';
              if (device?.status === 'warning') return 'hsl(38, 92%, 55%)';
              if (device?.status === 'offline') return 'hsl(0, 84%, 60%)';
              return 'hsl(0, 0%, 30%)';
            }}
            style={{ backgroundColor: 'hsl(0, 0%, 5%)' }}
          />
        </ReactFlow>

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
}

export const FleetGraph = () => (
  <ReactFlowProvider>
    <FleetGraphInner />
  </ReactFlowProvider>
);
