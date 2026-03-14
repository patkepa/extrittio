import { useCallback, useEffect, useRef } from 'react';
import type { GraphData, GraphNode } from './build-force-graph-data';
import type { ViewportInfo } from './fleet-graph-minimap';
import type { GraphActions } from './fleet-graph-canvas';

export function useViewportControls(
  graphRef: React.MutableRefObject<any>,
  canvasWrapperRef: React.RefObject<HTMLDivElement | null>,
  graphData: GraphData,
  width: number,
  height: number,
  hasInitialFit: React.MutableRefObject<boolean>,
  selectedNodeId?: string | null,
  onViewportChange?: (transform: ViewportInfo) => void,
  graphActionsRef?: React.MutableRefObject<GraphActions | null>,
) {
  // --- Viewport clamping via __zoom property interceptor ---
  const nodeBoundsRef = useRef<{
    centerX: number;
    centerY: number;
    padX: number;
    padY: number;
  } | null>(null);
  const dimensionsRef = useRef({ width, height });
  dimensionsRef.current = { width, height };

  // Recompute bounds when node positions settle
  const updateNodeBounds = useCallback(() => {
    const positioned = graphData.nodes.filter((n: GraphNode) => n.x != null && n.y != null);
    if (positioned.length === 0) {
      nodeBoundsRef.current = null;
      return;
    }

    let minX = Infinity,
      minY = Infinity,
      maxX = -Infinity,
      maxY = -Infinity;
    for (const n of positioned) {
      if (n.x! < minX) minX = n.x!;
      if (n.y! < minY) minY = n.y!;
      if (n.x! > maxX) maxX = n.x!;
      if (n.y! > maxY) maxY = n.y!;
    }
    const rangeX = maxX - minX || 200;
    const rangeY = maxY - minY || 200;
    nodeBoundsRef.current = {
      centerX: (minX + maxX) / 2,
      centerY: (minY + maxY) / 2,
      padX: Math.max(rangeX * 0.8, 300),
      padY: Math.max(rangeY * 0.8, 300),
    };
  }, [graphData.nodes]);

  useEffect(updateNodeBounds, [updateNodeBounds]);

  // Install the __zoom interceptor on the canvas element
  useEffect(() => {
    const canvas = canvasWrapperRef.current?.querySelector('canvas');
    if (!canvas) return;

    // Grab the existing transform value that d3-zoom already set
    let currentZoom = (canvas as any).__zoom;

    Object.defineProperty(canvas, '__zoom', {
      configurable: true,
      enumerable: true,
      get() {
        return currentZoom;
      },
      set(val) {
        const bounds = nodeBoundsRef.current;
        const { width: w, height: h } = dimensionsRef.current;
        if (!hasInitialFit.current || !bounds || !val) {
          currentZoom = val;
          return;
        }

        const k = val.k;
        const viewCenterX = (w / 2 - val.x) / k;
        const viewCenterY = (h / 2 - val.y) / k;

        const clampedX = Math.max(
          bounds.centerX - bounds.padX,
          Math.min(bounds.centerX + bounds.padX, viewCenterX),
        );
        const clampedY = Math.max(
          bounds.centerY - bounds.padY,
          Math.min(bounds.centerY + bounds.padY, viewCenterY),
        );

        if (clampedX !== viewCenterX || clampedY !== viewCenterY) {
          const newTx = w / 2 - clampedX * k;
          const newTy = h / 2 - clampedY * k;
          currentZoom = new val.constructor(k, newTx, newTy);
        } else {
          currentZoom = val;
        }
      },
    });

    return () => {
      // Restore a normal data property on cleanup
      Object.defineProperty(canvas, '__zoom', {
        configurable: true,
        writable: true,
        enumerable: true,
        value: currentZoom,
      });
    };
  }, [canvasWrapperRef, hasInitialFit]); // Canvas element is stable — refs provide latest values

  // Expose graph actions via ref
  useEffect(() => {
    if (!graphActionsRef) return;
    graphActionsRef.current = {
      navigateTo: (x, y) => graphRef.current?.centerAt(x, y, 500),
      fitView: () => graphRef.current?.zoomToFit(400, 60),
      zoomIn: () => {
        const fg = graphRef.current;
        if (!fg) return;
        const cur = fg.zoom();
        fg.zoom(Math.min(cur * 1.4, 8), 300);
      },
      zoomOut: () => {
        const fg = graphRef.current;
        if (!fg) return;
        const cur = fg.zoom();
        fg.zoom(Math.max(cur / 1.4, 0.5), 300);
      },
    };
  }, [graphRef, graphActionsRef]);

  // Forward viewport changes to parent (for minimap)
  const handleZoom = useCallback(
    (transform: { k: number; x: number; y: number }) => {
      onViewportChange?.(transform);
    },
    [onViewportChange],
  );

  // Center on selected node
  useEffect(() => {
    if (!selectedNodeId || !graphRef.current) return;
    const node = graphData.nodes.find((n: GraphNode) => n.id === selectedNodeId);
    if (node?.x != null && node?.y != null) {
      graphRef.current.centerAt(node.x, node.y, 500);
      graphRef.current.zoom(2, 500);
    }
  }, [selectedNodeId, graphData.nodes, graphRef]);

  return { updateNodeBounds, handleZoom };
}
