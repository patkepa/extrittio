import { useCallback, useEffect } from 'react';
// @ts-expect-error — d3-force-3d ships as transitive dep without types
import { forceCollide } from 'd3-force-3d';
import type { GraphNode } from './build-force-graph-data';

const FLEET_RADIUS = 14;
const DEVICE_RADIUS = 11;

export function useForceSimulation(
  graphRef: React.MutableRefObject<any>,
  updateNodeBounds: () => void,
  hasInitialFit: React.MutableRefObject<boolean>,
) {
  // Configure forces after mount
  useEffect(() => {
    if (!graphRef.current) return;
    const fg = graphRef.current;
    fg.d3Force('charge').strength(-30);
    fg.d3Force('link').distance(80);
    fg.d3Force(
      'collide',
      forceCollide((node: GraphNode) =>
        node.type === 'fleet' ? FLEET_RADIUS + 6 : DEVICE_RADIUS + 4,
      ),
    );
  }, [graphRef]);

  // Fit to view only on initial simulation settle
  const handleEngineStop = useCallback(() => {
    if (!hasInitialFit.current) {
      hasInitialFit.current = true;
      graphRef.current?.zoomToFit(400, 60);
    }
    updateNodeBounds();
  }, [graphRef, updateNodeBounds, hasInitialFit]);

  return { handleEngineStop };
}
