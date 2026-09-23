import { loadIconPaths, normalizeIconName, DEFAULT_DEVICE_TYPE_ICON } from './graph-icons';
import { formatExternalTooltip } from './graph-tooltips';
import { paintGraphNode, paintGraphLink, type DeviceAlertBadge } from './graph-renderers';
import { paintNodeHitArea } from './node-geometry';
export type { DeviceAlertBadge, AlertSeverity } from './graph-renderers';
import { memo, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import ForceGraph2D from 'react-force-graph-2d';
import type { GraphData, GraphNode, GraphLink } from './build-force-graph-data';
import type { Device } from '../../types/api';
import type { ViewportInfo } from './fleet-graph-minimap';
import { useForceSimulation } from './use-force-simulation';
import { useLassoSelection } from './use-lasso-selection';
import { useViewportControls } from './use-viewport-controls';
import type { ForceGraphApi } from './force-graph-types';

export interface GraphActions {
  navigateTo: (x: number, y: number, durationMs?: number) => void;
  fitView: () => void;
  zoomIn: () => void;
  zoomOut: () => void;
}

// --- Constants ---
const GRID_SIZE = 40;
const GRID_COLOR = 'rgba(255, 255, 255, 0.05)';
const GRID_ACCENT_COLOR = 'rgba(255, 255, 255, 0.12)';
const GRID_ACCENT_EVERY = 5; // every 5th line is brighter
// Click-vs-drag threshold (px). The library's internal threshold is only 5 px
// and its background-pan detection has *zero* tolerance for mouse events, so
// fast-approach clicks are swallowed as drags. We bypass the library's click
// handling entirely and use this more generous threshold instead.
const CLICK_DIST_THRESHOLD = 12;

interface FleetGraphCanvasProps {
  graphData: GraphData;
  width: number;
  height: number;
  onNodeClick?: (device: Device, position: { x: number; y: number }) => void;
  onGraphNodeClick?: (node: GraphNode, position: { x: number; y: number }) => void;
  onGraphNodeDragEnd?: (node: GraphNode) => void;
  onBackgroundClick: (event?: MouseEvent) => void;
  onNodeRightClick?: (node: GraphNode, event: MouseEvent) => void;
  selectedNodeId?: string | null;
  hoveredNodeId?: string | null;
  onViewportChange?: (transform: ViewportInfo) => void;
  graphActionsRef?: React.MutableRefObject<GraphActions | null>;
  onFrameRedraw?: () => void;
  showDeviceLabels?: boolean;
  showAlertBadges?: boolean;
  alertBadges?: Record<string, DeviceAlertBadge>;
}

export const FleetGraphCanvas = memo(
  ({
    graphData,
    width,
    height,
    onNodeClick,
    onGraphNodeClick,
    onGraphNodeDragEnd,
    onBackgroundClick,
    onNodeRightClick,
    selectedNodeId,
    hoveredNodeId,
    onViewportChange,
    graphActionsRef,
    onFrameRedraw,
    showDeviceLabels = true,
    showAlertBadges = true,
    alertBadges = {},
  }: FleetGraphCanvasProps) => {
    const graphRef = useRef<ForceGraphApi>(undefined);
    const [hoverNode, setHoverNode] = useState<GraphNode | null>(null);
    const [iconCacheVersion, setIconCacheVersion] = useState(0);
    const pulseClockRef = useRef(0);
    const hoverTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
    const canvasWrapperRef = useRef<HTMLDivElement>(null);
    const hasInitialFit = useRef(false);

    // Immediate (non-debounced) hover ref — used for click-target detection so
    // that fast-approach clicks always know which node the pointer is over,
    // even before the debounced visual state has updated.
    const hoverNodeRef = useRef<GraphNode | null>(null);

    // Pointer-down state for our custom click detection.
    const pointerStartRef = useRef<{
      x: number;
      y: number;
      node: GraphNode | null;
    } | null>(null);

    // Whether a node is currently being dragged (for cursor management).
    const isDraggingRef = useRef(false);
    const didDragNodeRef = useRef(false);

    // --- Custom hooks ---
    const { updateNodeBounds, handleZoom } = useViewportControls(
      graphRef,
      canvasWrapperRef,
      graphData,
      width,
      height,
      hasInitialFit,
      selectedNodeId,
      onViewportChange,
      graphActionsRef,
    );

    const { handleEngineStop } = useForceSimulation(
      graphRef,
      graphData,
      updateNodeBounds,
      hasInitialFit,
    );

    const { shiftHeld, selectedDeviceIds, toggleDevice, clearSelection, paintLasso } =
      useLassoSelection(graphRef, canvasWrapperRef, graphData);

    const activeHoverNode = useMemo(() => {
      if (!hoveredNodeId) return hoverNode;
      return graphData.nodes.find((node) => node.id === hoveredNodeId) ?? null;
    }, [graphData.nodes, hoverNode, hoveredNodeId]);

    useEffect(() => {
      let cancelled = false;
      const icons = new Set<string>([DEFAULT_DEVICE_TYPE_ICON]);
      for (const node of graphData.nodes) {
        icons.add(normalizeIconName(node.visualIcon));
      }

      void Promise.all(Array.from(icons, (icon) => loadIconPaths(icon)))
        .then(() => {
          if (!cancelled) {
            setIconCacheVersion((version) => version + 1);
          }
        })
        .catch((error: unknown) => {
          console.error('[FleetGraph] Failed to load node icons', error);
        });

      return () => {
        cancelled = true;
      };
    }, [graphData.nodes]);

    const hoverHighlight = useMemo(() => {
      const nodes = new Set<GraphNode>();
      const links = new Set<GraphLink>();

      if (activeHoverNode) {
        nodes.add(activeHoverNode);
        activeHoverNode.neighbors.forEach((node) => nodes.add(node));
        activeHoverNode.links.forEach((link) => links.add(link));
      }

      return { nodes, links };
    }, [activeHoverNode]);

    useEffect(() => {
      let rafId: number;
      const tick = () => {
        pulseClockRef.current = performance.now();
        rafId = requestAnimationFrame(tick);
      };
      rafId = requestAnimationFrame(tick);
      return () => cancelAnimationFrame(rafId);
    }, []);

    // Suppress browser's native context menu
    useEffect(() => {
      const wrapper = canvasWrapperRef.current;
      if (!wrapper) return;

      const suppress = (e: Event) => e.preventDefault();
      wrapper.addEventListener('contextmenu', suppress);
      return () => wrapper.removeEventListener('contextmenu', suppress);
    }, []);

    // Sync canvas cursor — respects drag state so the grab cursor shows
    // correctly. The library's own .grabbable class is overridden by the
    // inline style we set here, so we must manage all cursor states ourselves.
    useEffect(() => {
      if (isDraggingRef.current) return; // don't clobber the drag cursor
      const canvas = canvasWrapperRef.current?.querySelector('canvas');
      if (!canvas) return;
      if (shiftHeld) {
        canvas.style.cursor = 'crosshair';
      } else if (hoverNode) {
        canvas.style.cursor = 'pointer';
      } else {
        canvas.style.cursor = 'default';
      }
    }, [hoverNode, shiftHeld]);

    // Node drag cursor handlers — set grabbing cursor immediately (without
    // waiting for a React re-render) and restore the correct cursor on end.
    const handleNodeDrag = useCallback(() => {
      didDragNodeRef.current = true;
      if (!isDraggingRef.current) {
        isDraggingRef.current = true;
        const canvas = canvasWrapperRef.current?.querySelector('canvas');
        if (canvas) canvas.style.cursor = 'grabbing';
      }
    }, []);

    const handleNodeDragEnd = useCallback(
      (node: GraphNode) => {
        // Move the node's gentle force target along with it. Without this, the
        // layout force immediately pulls a released node back to its old anchor,
        // which makes dragging feel broken even though the pointer interaction
        // itself succeeded.
        if (node.x != null && node.y != null) {
          node.layoutX = node.x;
          node.layoutY = node.y;
        }
        onGraphNodeDragEnd?.(node);

        isDraggingRef.current = false;
        const canvas = canvasWrapperRef.current?.querySelector('canvas');
        if (!canvas) return;
        canvas.style.cursor = hoverNodeRef.current ? 'pointer' : 'default';
      },
      [onGraphNodeDragEnd],
    );

    // Hover handler — debounced to avoid flickering when quickly brushing over nodes.
    // The raw ref is updated immediately so click detection always has the
    // up-to-date hover target even before the debounced state fires.
    const handleNodeHover = useCallback((node: GraphNode | null) => {
      hoverNodeRef.current = node;

      if (hoverTimerRef.current) {
        clearTimeout(hoverTimerRef.current);
        hoverTimerRef.current = null;
      }

      if (!node) {
        setHoverNode(null);
        return;
      }

      hoverTimerRef.current = setTimeout(() => {
        setHoverNode(node);
      }, 15);
    }, []);

    useEffect(() => {
      return () => {
        if (hoverTimerRef.current) {
          clearTimeout(hoverTimerRef.current);
        }
      };
    }, []);

    // --- Custom click detection ---
    // We bypass the library's onNodeClick / onBackgroundClick entirely because
    // its internal drag-vs-click heuristic has zero tolerance for mouse
    // movement on background-pan detection (any pointermove while pressed →
    // isPointerDragging=true → click suppressed). This makes fast-approach
    // clicks unreliable. Our wrapper-level handler uses a generous distance
    // threshold so quick cursor settling doesn't eat the click.
    //
    // Removing onBackgroundClick from ForceGraph2D also disables the
    // library's aggressive zero-tolerance drag guard (it's gated behind
    // `state.onBackgroundClick` being truthy).

    const handleWrapperPointerDown = useCallback((e: React.PointerEvent) => {
      if (e.button !== 0) return; // left-click only
      didDragNodeRef.current = false;
      pointerStartRef.current = {
        x: e.clientX,
        y: e.clientY,
        node: hoverNodeRef.current,
      };
    }, []);

    const handleWrapperPointerUp = useCallback(
      (e: React.PointerEvent) => {
        if (e.button !== 0) return;
        const start = pointerStartRef.current;
        pointerStartRef.current = null;
        if (!start) return;
        if (didDragNodeRef.current) return;

        const dx = e.clientX - start.x;
        const dy = e.clientY - start.y;
        if (dx * dx + dy * dy > CLICK_DIST_THRESHOLD * CLICK_DIST_THRESHOLD) return;

        // Prefer the node captured at pointer-down (guaranteed to be the
        // intended target even if hover drifted during the gesture), but
        // fall back to the current hover for edge-cases where the pointer
        // landed on the node between frames.
        const node = start.node ?? hoverNodeRef.current;

        if (node) {
          if (onGraphNodeClick) {
            onGraphNodeClick(node, { x: e.clientX, y: e.clientY });
          } else if (e.shiftKey && node.type === 'device') {
            toggleDevice(node.id);
          } else if (node.type === 'device' && node.device && onNodeClick) {
            onNodeClick(node.device, { x: e.clientX, y: e.clientY });
          } else {
            // Fleet hub node or unknown — treat as background
            onBackgroundClick();
          }
        } else {
          if (!e.shiftKey) {
            clearSelection();
          }
          onBackgroundClick();
        }
      },
      [onGraphNodeClick, onNodeClick, onBackgroundClick, toggleDevice, clearSelection],
    );

    // --- Pointer hit-area callback ---
    // The library detects hover/click by painting each node in a unique color
    // on a hidden shadow canvas, then sampling the pixel under the cursor.
    // Without this, the default hit area is Math.sqrt(val)*nodeRelSize (=8px
    // for devices), which is smaller than the visual radius (11px, or 14.3px
    // on hover). This mismatch causes clicks to miss, especially during
    // the hover-expand transition. We paint the hit area at the *expanded*
    // size so clicks always register on the visible area.
    const paintPointerArea = paintNodeHitArea;

    // --- Canvas rendering callbacks ---
    const renderState = useMemo(
      () => ({
        activeHoverNode,
        selectedNodeId,
        hoverHighlight,
        iconCacheVersion,
        selectedDeviceIds,
        showDeviceLabels,
        showAlertBadges,
        alertBadges,
      }),
      [
        activeHoverNode,
        alertBadges,
        hoverHighlight,
        iconCacheVersion,
        selectedDeviceIds,
        selectedNodeId,
        showAlertBadges,
        showDeviceLabels,
      ],
    );

    const paintNode = useCallback(
      (node: GraphNode, ctx: CanvasRenderingContext2D, globalScale: number) => {
        paintGraphNode(node, ctx, globalScale, renderState, pulseClockRef.current);
      },
      [renderState],
    );

    const linkRenderState = useMemo(
      () => ({ activeHoverNode, hoverHighlight }),
      [activeHoverNode, hoverHighlight],
    );
    const paintLink = useCallback(
      (link: GraphLink, ctx: CanvasRenderingContext2D) => {
        paintGraphLink(link, ctx, linkRenderState, pulseClockRef.current);
      },
      [linkRenderState],
    );

    // Draw a grid in world-space
    const paintGrid = useCallback(
      (ctx: CanvasRenderingContext2D, globalScale: number) => {
        const fg = graphRef.current;
        if (!fg) return;

        const topLeft = fg.screen2GraphCoords(0, 0);
        const bottomRight = fg.screen2GraphCoords(width, height);

        const step = GRID_SIZE;
        const startX = Math.floor(topLeft.x / step) * step;
        const startY = Math.floor(topLeft.y / step) * step;
        const endX = Math.ceil(bottomRight.x / step) * step;
        const endY = Math.ceil(bottomRight.y / step) * step;

        for (let x = startX; x <= endX; x += step) {
          const gridIdx = Math.round(x / step);
          ctx.strokeStyle = gridIdx % GRID_ACCENT_EVERY === 0 ? GRID_ACCENT_COLOR : GRID_COLOR;
          ctx.lineWidth = 1 / globalScale;
          ctx.beginPath();
          ctx.moveTo(x, topLeft.y);
          ctx.lineTo(x, bottomRight.y);
          ctx.stroke();
        }

        for (let y = startY; y <= endY; y += step) {
          const gridIdx = Math.round(y / step);
          ctx.strokeStyle = gridIdx % GRID_ACCENT_EVERY === 0 ? GRID_ACCENT_COLOR : GRID_COLOR;
          ctx.lineWidth = 1 / globalScale;
          ctx.beginPath();
          ctx.moveTo(topLeft.x, y);
          ctx.lineTo(bottomRight.x, y);
          ctx.stroke();
        }
      },
      [width, height],
    );

    const handleRenderFramePost = useCallback(
      (ctx: CanvasRenderingContext2D, globalScale: number) => {
        paintLasso(ctx, globalScale);
        onFrameRedraw?.();
      },
      [paintLasso, onFrameRedraw],
    );

    const getNodeLabel = useCallback((node: GraphNode) => {
      if (node.type !== 'external' && !node.details) return '';
      return formatExternalTooltip(node);
    }, []);

    return (
      <div
        ref={canvasWrapperRef}
        style={{ width, height }}
        onPointerDown={handleWrapperPointerDown}
        onPointerUp={handleWrapperPointerUp}
      >
        <ForceGraph2D<GraphNode, GraphLink>
          ref={graphRef}
          graphData={graphData}
          width={width}
          height={height}
          backgroundColor="#171717"
          onRenderFramePre={paintGrid}
          onRenderFramePost={handleRenderFramePost}
          nodeCanvasObject={paintNode}
          nodeCanvasObjectMode={() => 'replace'}
          linkCanvasObject={paintLink}
          linkCanvasObjectMode={() => 'replace'}
          nodePointerAreaPaint={paintPointerArea}
          onNodeHover={handleNodeHover}
          onNodeDrag={handleNodeDrag}
          onNodeDragEnd={handleNodeDragEnd}
          onNodeRightClick={onNodeRightClick}
          onEngineStop={handleEngineStop}
          enablePanInteraction={!shiftHeld}
          enableNodeDrag={!shiftHeld}
          nodeVal="val"
          d3AlphaDecay={0.02}
          d3VelocityDecay={0.3}
          warmupTicks={50}
          cooldownTicks={200}
          autoPauseRedraw={false}
          nodeLabel={getNodeLabel}
          onZoom={handleZoom}
          minZoom={0.5}
          maxZoom={8}
        />
      </div>
    );
  },
);
