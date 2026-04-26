/* eslint-disable @typescript-eslint/no-explicit-any -- react-force-graph-2d lacks proper TS types */
import { memo, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import ForceGraph2D from 'react-force-graph-2d';
import type { GraphData, GraphNode, GraphLink } from './build-force-graph-data';
import type { Device } from '../../types/api';
import { getHealthTier, getStalenessColor, getPulseFrequency } from './health-utils';
import { TIER_COLORS, TYPE_ICON_PATHS, FALLBACK_ICON_PATHS, SELECTION_COLOR } from './constants';
import type { ViewportInfo } from './fleet-graph-minimap';
import { useForceSimulation } from './use-force-simulation';
import { useLassoSelection } from './use-lasso-selection';
import { useViewportControls } from './use-viewport-controls';

export interface GraphActions {
  navigateTo: (x: number, y: number) => void;
  fitView: () => void;
  zoomIn: () => void;
  zoomOut: () => void;
}

// --- Constants ---
const FLEET_RADIUS = 14;
const DEVICE_RADIUS = 11;
const HOVER_SCALE = 1.3;
const DIM_OPACITY = 0.15;
const FLEET_LABEL_FONT = 'bold 10px -apple-system, BlinkMacSystemFont, sans-serif';
const GRID_SIZE = 40;
const GRID_COLOR = 'rgba(255, 255, 255, 0.05)';
const GRID_ACCENT_COLOR = 'rgba(255, 255, 255, 0.12)';
const GRID_ACCENT_EVERY = 5; // every 5th line is brighter
// Click-vs-drag threshold (px). The library's internal threshold is only 5 px
// and its background-pan detection has *zero* tolerance for mouse events, so
// fast-approach clicks are swallowed as drags. We bypass the library's click
// handling entirely and use this more generous threshold instead.
const CLICK_DIST_THRESHOLD = 12;

// --- Pre-built Path2D cache for device-type icons (16×16 viewBox) ---
const iconPathCache = new Map<string, Path2D[]>();

function getIconPaths(deviceTypeName?: string): Path2D[] {
  const key = deviceTypeName?.toLowerCase() ?? '__fallback__';
  let cached = iconPathCache.get(key);
  if (cached) return cached;

  const svgPaths = TYPE_ICON_PATHS[key] ?? FALLBACK_ICON_PATHS;
  cached = svgPaths.map((d) => new Path2D(d));
  iconPathCache.set(key, cached);
  return cached;
}

/** Draw a Blueprint icon (16×16 paths) centered at (cx, cy), scaled to fit `size`. */
function drawIcon(
  ctx: CanvasRenderingContext2D,
  paths: Path2D[],
  cx: number,
  cy: number,
  size: number,
) {
  const scale = size / 16;
  ctx.save();
  // Translate so the 16×16 icon is centered at (cx, cy)
  ctx.translate(cx - size / 2, cy - size / 2);
  ctx.scale(scale, scale);
  for (const p of paths) {
    ctx.fill(p);
  }
  ctx.restore();
}

interface FleetGraphCanvasProps {
  graphData: GraphData;
  width: number;
  height: number;
  onNodeClick: (device: Device, position: { x: number; y: number }) => void;
  onBackgroundClick: (event?: MouseEvent) => void;
  onNodeRightClick?: (node: GraphNode, event: MouseEvent) => void;
  selectedNodeId?: string | null;
  hoveredNodeId?: string | null;
  onViewportChange?: (transform: ViewportInfo) => void;
  graphActionsRef?: React.MutableRefObject<GraphActions | null>;
  onFrameRedraw?: () => void;
}

export const FleetGraphCanvas = memo(
  ({
    graphData,
    width,
    height,
    onNodeClick,
    onBackgroundClick,
    onNodeRightClick,
    selectedNodeId,
    hoveredNodeId,
    onViewportChange,
    graphActionsRef,
    onFrameRedraw,
  }: FleetGraphCanvasProps) => {
    const graphRef = useRef<any>(null);
    const [hoverNode, setHoverNode] = useState<GraphNode | null>(null);
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
      updateNodeBounds,
      hasInitialFit,
    );

    const {
      shiftHeld,
      selectedDeviceIds,
      toggleDevice,
      clearSelection,
      enablePanInteraction,
      paintLasso,
    } = useLassoSelection(graphRef, canvasWrapperRef, graphData);

    const activeHoverNode = useMemo(() => {
      if (!hoveredNodeId) return hoverNode;
      return graphData.nodes.find((node) => node.id === hoveredNodeId) ?? null;
    }, [graphData.nodes, hoverNode, hoveredNodeId]);

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

    // Sync canvas cursor to pointer only when over a node
    useEffect(() => {
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

    const handleWrapperPointerDown = useCallback(
      (e: React.PointerEvent) => {
        if (e.button !== 0) return; // left-click only
        pointerStartRef.current = {
          x: e.clientX,
          y: e.clientY,
          node: hoverNodeRef.current,
        };
      },
      [],
    );

    const handleWrapperPointerUp = useCallback(
      (e: React.PointerEvent) => {
        if (e.button !== 0) return;
        const start = pointerStartRef.current;
        pointerStartRef.current = null;
        if (!start) return;

        const dx = e.clientX - start.x;
        const dy = e.clientY - start.y;
        if (dx * dx + dy * dy > CLICK_DIST_THRESHOLD * CLICK_DIST_THRESHOLD) return;

        // Prefer the node captured at pointer-down (guaranteed to be the
        // intended target even if hover drifted during the gesture), but
        // fall back to the current hover for edge-cases where the pointer
        // landed on the node between frames.
        const node = start.node ?? hoverNodeRef.current;

        if (node) {
          if (e.shiftKey && node.type === 'device') {
            toggleDevice(node.id);
          } else if (node.type === 'device' && node.device) {
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
      [onNodeClick, onBackgroundClick, toggleDevice, clearSelection],
    );

    // --- Pointer hit-area callback ---
    // The library detects hover/click by painting each node in a unique color
    // on a hidden shadow canvas, then sampling the pixel under the cursor.
    // Without this, the default hit area is Math.sqrt(val)*nodeRelSize (=8px
    // for devices), which is smaller than the visual radius (11px, or 14.3px
    // on hover). This mismatch causes clicks to miss, especially during
    // the hover-expand transition. We paint the hit area at the *expanded*
    // size so clicks always register on the visible area.
    const paintPointerArea = useCallback(
      (node: GraphNode, color: string, ctx: CanvasRenderingContext2D) => {
        if (node.x == null || node.y == null) return;

        if (node.type === 'fleet') {
          // Fleet nodes are rendered as label rectangles — approximate the
          // clickable area with a generous rectangle matching the visual.
          const padX = 8;
          const padY = 4;
          const barH = 3;
          ctx.font = FLEET_LABEL_FONT;
          const textWidth = ctx.measureText(node.name).width;
          // Always use the hover-expanded size for the hit area
          const hoverScale = 1.1;
          const rectW = (textWidth + padX * 2) * hoverScale;
          const rectH = (FLEET_RADIUS + padY + barH) * hoverScale;

          ctx.fillStyle = color;
          ctx.fillRect(node.x - rectW / 2, node.y - rectH / 2, rectW, rectH);
        } else {
          // Device nodes — use hover-expanded radius so the click area
          // always covers the visual, even mid-expansion.
          const radius = DEVICE_RADIUS * HOVER_SCALE;
          const side = radius * 2;

          ctx.fillStyle = color;
          ctx.fillRect(node.x - side / 2, node.y - side / 2, side, side);
        }
      },
      [],
    );

    // --- Canvas rendering callbacks ---
    const paintNode = useCallback(
      (node: GraphNode, ctx: CanvasRenderingContext2D, globalScale: number) => {
        const isFleet = node.type === 'fleet';
        const baseRadius = isFleet ? FLEET_RADIUS : DEVICE_RADIUS;
        const isHovered = node === activeHoverNode;
        const isHighlighted = hoverHighlight.nodes.has(node);
        const shouldDim = activeHoverNode && !isHighlighted;

        if (node.x == null || node.y == null) return;

        ctx.globalAlpha = shouldDim ? DIM_OPACITY : 1;

        const radius = isHovered ? baseRadius * HOVER_SCALE : baseRadius;

        if (!isFleet && isHovered) {
          ctx.shadowColor = node.color;
          ctx.shadowBlur = 20;
        } else if (!isFleet && isHighlighted) {
          ctx.shadowColor = node.color;
          ctx.shadowBlur = 10;
        } else {
          ctx.shadowBlur = 0;
        }

        if (isFleet) {
          const padX = 8;
          const padY = 4;
          const barH = 3;
          const hoverScale = isHovered ? 1.1 : 1;

          ctx.font = FLEET_LABEL_FONT;
          const textWidth = ctx.measureText(node.name).width;
          const rectW = (textWidth + padX * 2) * hoverScale;
          const rectH = (FLEET_RADIUS + padY + barH) * hoverScale;
          const rx = node.x! - rectW / 2;
          const ry = node.y! - rectH / 2;

          ctx.fillStyle = node.color;
          ctx.fillRect(rx, ry, rectW, rectH);

          ctx.shadowBlur = 0;

          if (node.tierRatios) {
            const scaledBarH = barH * hoverScale;
            const barY = ry + rectH - scaledBarH;
            const segments: [number, string][] = [
              [node.tierRatios.fresh, TIER_COLORS.fresh],
              [node.tierRatios.warm, TIER_COLORS.warm],
              [node.tierRatios.stale, TIER_COLORS.stale],
              [node.tierRatios.dead, TIER_COLORS.dead],
              [node.tierRatios.never, TIER_COLORS.never],
            ];
            let offsetX = 0;
            for (const [ratio, color] of segments) {
              if (ratio <= 0) continue;
              const segW = ratio * rectW;
              ctx.fillStyle = color;
              ctx.fillRect(rx + offsetX, barY, segW, scaledBarH);
              offsetX += segW;
            }

            ctx.strokeStyle = 'rgba(0,0,0,0.6)';
            ctx.lineWidth = 0.5;
            ctx.beginPath();
            ctx.moveTo(rx, barY);
            ctx.lineTo(rx + rectW, barY);
            ctx.stroke();
          }

          ctx.strokeStyle = 'rgba(0,0,0,0.6)';
          ctx.lineWidth = 0.5;
          ctx.strokeRect(rx, ry, rectW, rectH);

          ctx.font = FLEET_LABEL_FONT;
          ctx.textAlign = 'center';
          ctx.textBaseline = 'middle';
          ctx.fillStyle = '#ffffff';
          ctx.fillText(node.name, node.x!, node.y! - (barH * hoverScale) / 2);

          if (node.deviceCount != null) {
            ctx.font = '8px -apple-system, sans-serif';
            ctx.fillStyle = 'rgba(255,255,255,0.6)';
            ctx.fillText(
              `${node.deviceCount} device${node.deviceCount === 1 ? '' : 's'}`,
              node.x!,
              node.y! + rectH / 2 + 10,
            );
          }
        } else {
          ctx.shadowBlur = 0;
          const now = Date.now();
          const stalenessMs = node.lastSeenTimestamp ? now - node.lastSeenTimestamp : NaN;
          const tier = getHealthTier(stalenessMs, node.status);
          const stalenessColor = getStalenessColor(stalenessMs, node.status);
          const pulseHz = getPulseFrequency(tier);

          const effectiveRadius = tier === 'dead' || tier === 'never' ? radius * 0.85 : radius;
          const side = effectiveRadius * 2;
          const rx = node.x! - side / 2;
          const ry = node.y! - side / 2;

          if (pulseHz > 0) {
            const t = pulseClockRef.current / 1000;
            const glowRadius = effectiveRadius + 4 + 4 * Math.sin(2 * Math.PI * pulseHz * t);
            const gradient = ctx.createRadialGradient(
              node.x!,
              node.y!,
              effectiveRadius,
              node.x!,
              node.y!,
              glowRadius,
            );
            const r = parseInt(stalenessColor.slice(1, 3), 16);
            const g = parseInt(stalenessColor.slice(3, 5), 16);
            const b = parseInt(stalenessColor.slice(5, 7), 16);
            gradient.addColorStop(0, `rgba(${r},${g},${b},0.5)`);
            gradient.addColorStop(1, `rgba(${r},${g},${b},0)`);
            ctx.beginPath();
            ctx.arc(node.x!, node.y!, glowRadius, 0, 2 * Math.PI);
            ctx.fillStyle = gradient;
            ctx.fill();
          }

          ctx.fillStyle = stalenessColor;
          ctx.fillRect(rx, ry, side, side);

          if (node.uptimeArcAngle && node.uptimeArcAngle > 0) {
            const ringOffset = 3;
            ctx.strokeStyle = stalenessColor;
            ctx.lineWidth = 2;
            ctx.strokeRect(
              rx - ringOffset,
              ry - ringOffset,
              side + ringOffset * 2,
              side + ringOffset * 2,
            );
          }

          if (selectedDeviceIds.has(node.id)) {
            const selOffset = 7;
            ctx.strokeStyle = SELECTION_COLOR;
            ctx.lineWidth = 2;
            ctx.strokeRect(
              rx - selOffset,
              ry - selOffset,
              side + selOffset * 2,
              side + selOffset * 2,
            );
          }

          const iconPaths = getIconPaths(node.deviceTypeName);
          const iconSize = effectiveRadius * 1.2;
          ctx.fillStyle = '#ffffff';
          drawIcon(ctx, iconPaths, node.x!, node.y!, iconSize);

          const fontSize = Math.max(10, 12 / globalScale);
          ctx.font = `${fontSize}px -apple-system, sans-serif`;
          ctx.fillStyle = shouldDim ? `rgba(255,255,255,${DIM_OPACITY})` : 'rgba(255,255,255,0.8)';
          ctx.fillText(node.name, node.x!, node.y! + effectiveRadius + fontSize + 2);
        }

        ctx.globalAlpha = 1;
      },
      [activeHoverNode, hoverHighlight, selectedDeviceIds],
    );

    const paintLink = useCallback(
      (link: GraphLink, ctx: CanvasRenderingContext2D) => {
        const isHighlighted = hoverHighlight.links.has(link);
        const shouldDim = activeHoverNode && !isHighlighted;

        const source = link.source as any as GraphNode;
        const target = link.target as any as GraphNode;
        if (source.x == null || target.x == null) return;

        const deviceNode =
          source.type === 'device' ? source : target.type === 'device' ? target : null;
        const isActive = deviceNode?.status === 'online' || deviceNode?.status === 'warning';
        const isNever =
          deviceNode != null && !deviceNode.lastSeenTimestamp && deviceNode.status === 'offline';

        ctx.beginPath();
        if (isActive) {
          ctx.setLineDash([4, 4]);
          ctx.lineDashOffset = -(pulseClockRef.current / 1000) * 12;
        } else {
          ctx.setLineDash([3, 5]);
        }
        ctx.moveTo(source.x!, source.y!);
        ctx.lineTo(target.x!, target.y!);

        if (isHighlighted) {
          const hlColor = isActive
            ? 'rgba(0, 200, 80, 0.9)'
            : isNever
              ? 'rgba(92, 112, 128, 0.9)'
              : 'rgba(255, 60, 60, 0.8)';
          const hlGlow = isActive
            ? 'rgba(0, 200, 80, 0.4)'
            : isNever
              ? 'rgba(92, 112, 128, 0.4)'
              : 'rgba(255, 60, 60, 0.3)';
          ctx.strokeStyle = hlColor;
          ctx.lineWidth = 1.5;
          ctx.shadowColor = hlGlow;
          ctx.shadowBlur = 6;
        } else if (shouldDim) {
          const dimColor = isActive
            ? `rgba(0, 200, 80, ${DIM_OPACITY * 0.5})`
            : isNever
              ? `rgba(92, 112, 128, ${DIM_OPACITY * 0.5})`
              : `rgba(255, 60, 60, ${DIM_OPACITY * 0.5})`;
          ctx.strokeStyle = dimColor;
          ctx.lineWidth = 0.5;
          ctx.shadowBlur = 0;
        } else {
          ctx.strokeStyle = isActive
            ? 'rgba(0, 200, 80, 0.6)'
            : isNever
              ? 'rgba(92, 112, 128, 0.8)'
              : 'rgba(255, 60, 60, 0.45)';
          ctx.lineWidth = isActive ? 1 : isNever ? 0.8 : 0.5;
          ctx.shadowBlur = 0;
        }

        ctx.stroke();
        ctx.setLineDash([]);
        ctx.lineDashOffset = 0;
        ctx.shadowBlur = 0;
      },
      [activeHoverNode, hoverHighlight],
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

    return (
      <div
        ref={canvasWrapperRef}
        style={{ width, height }}
        onPointerDown={handleWrapperPointerDown}
        onPointerUp={handleWrapperPointerUp}
      >
        <ForceGraph2D
          ref={graphRef}
          graphData={graphData}
          width={width}
          height={height}
          backgroundColor="#171717"
          onRenderFramePre={paintGrid as any}
          onRenderFramePost={handleRenderFramePost as any}
          nodeCanvasObject={paintNode as any}
          nodeCanvasObjectMode={() => 'replace'}
          linkCanvasObject={paintLink as any}
          linkCanvasObjectMode={() => 'replace'}
          nodePointerAreaPaint={paintPointerArea as any}
          onNodeHover={handleNodeHover as any}
          onNodeRightClick={onNodeRightClick as any}
          onEngineStop={handleEngineStop}
          enablePanInteraction={enablePanInteraction as any}
          enableNodeDrag={!shiftHeld}
          nodeVal="val"
          d3AlphaDecay={0.02}
          d3VelocityDecay={0.3}
          warmupTicks={50}
          cooldownTicks={200}
          autoPauseRedraw={false}
          nodeLabel=""
          onZoom={handleZoom as any}
          minZoom={0.5}
          maxZoom={8}
        />
      </div>
    );
  },
);
