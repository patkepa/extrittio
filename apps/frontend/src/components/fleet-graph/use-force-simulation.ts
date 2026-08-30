import { useCallback, useEffect } from 'react';
// @ts-expect-error — d3-force-3d ships as transitive dep without types
import { forceCollide, forceX, forceY } from 'd3-force-3d';
import type { GraphData, GraphLink, GraphNode } from './build-force-graph-data';
import type { ForceGraphApi } from './force-graph-types';

const FLEET_RADIUS = 14;
const DEVICE_RADIUS = 11;
const EXTERNAL_RADIUS = 8;
const LEGACY_LINK_DISTANCE = 80;
const LEGACY_CHARGE_STRENGTH = -30;

interface InitialViewportControls {
  centerAt: (x?: number, y?: number, durationMs?: number) => unknown;
  zoomToFit: (durationMs?: number, padding?: number) => unknown;
}

export function applyInitialGraphViewport(
  graph: InitialViewportControls | undefined,
  nodes: readonly GraphNode[],
) {
  if (!graph) return;

  const viewportAnchor = nodes.find((node) => node.initialViewportAnchor);
  if (viewportAnchor?.x != null && viewportAnchor.y != null) {
    graph.centerAt(viewportAnchor.x, viewportAnchor.y, 400);
  } else {
    graph.zoomToFit(400, 60);
  }
}

function getLinkDevice(link: GraphLink): GraphNode | null {
  const source = link.source as GraphNode | string;
  const target = link.target as GraphNode | string;

  if (typeof source === 'object' && source.type === 'device') return source;
  if (typeof target === 'object' && target.type === 'device') return target;

  return null;
}

export function useForceSimulation(
  graphRef: React.MutableRefObject<ForceGraphApi | undefined>,
  graphData: GraphData,
  updateNodeBounds: () => void,
  hasInitialFitRef: React.MutableRefObject<boolean>,
) {
  // Configure forces after mount and whenever the graph topology changes.
  useEffect(() => {
    if (!graphRef.current) return;
    const fg = graphRef.current;

    fg.d3Force('charge')?.strength?.((node: GraphNode) => {
      if (node.layoutX == null || node.layoutY == null) return LEGACY_CHARGE_STRENGTH;
      if (node.type === 'external') return -20;
      return node.type === 'fleet' ? -120 : -55;
    });
    fg.d3Force('link')?.distance?.((link: GraphLink) => {
      if (link.layoutDistance != null) return link.layoutDistance;
      if (link.kind === 'declared') return 96;
      const device = getLinkDevice(link);
      return device?.layoutRadius ?? LEGACY_LINK_DISTANCE;
    });
    fg.d3Force('link')?.strength?.((link: GraphLink) => {
      if (link.layoutStrength != null) return link.layoutStrength;
      if (link.kind === 'declared') return 0.35;
      const device = getLinkDevice(link);
      return device?.layoutRadius == null ? 1 : 0.12;
    });
    fg.d3Force(
      'collide',
      forceCollide((node: GraphNode) => {
        if (node.layoutX == null || node.layoutY == null) {
          if (node.type === 'fleet') return FLEET_RADIUS + 6;
          if (node.type === 'external') return EXTERNAL_RADIUS + 7;
          return DEVICE_RADIUS + 4;
        }

        if (node.type === 'fleet') return FLEET_RADIUS + 12;
        if (node.type === 'external') return EXTERNAL_RADIUS + 8;
        return DEVICE_RADIUS + 12;
      }).iterations(2),
    );
    fg.d3Force(
      'layoutX',
      forceX((node: GraphNode) => node.layoutX ?? 0).strength((node: GraphNode) =>
        node.layoutX == null ? 0 : node.type === 'fleet' ? 0.035 : 0.025,
      ),
    );
    fg.d3Force(
      'layoutY',
      forceY((node: GraphNode) => node.layoutY ?? 0).strength((node: GraphNode) =>
        node.layoutY == null ? 0 : node.type === 'fleet' ? 0.035 : 0.025,
      ),
    );
    fg.d3ReheatSimulation();
  }, [graphData.nodes, graphData.links, graphRef]);

  // Establish the viewport only on initial simulation settle. Topology views
  // can nominate a local anchor so peripheral discoveries do not pull the
  // opening camera away from the node the operator controls.
  const handleEngineStop = useCallback(() => {
    if (!hasInitialFitRef.current) {
      hasInitialFitRef.current = true;
      applyInitialGraphViewport(graphRef.current, graphData.nodes);
    }
    updateNodeBounds();
  }, [graphData.nodes, graphRef, updateNodeBounds, hasInitialFitRef]);

  return { handleEngineStop };
}
