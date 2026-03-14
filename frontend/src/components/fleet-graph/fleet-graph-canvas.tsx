import { memo, useCallback, useEffect, useRef, useState } from 'react';
import ForceGraph2D from 'react-force-graph-2d';
// @ts-ignore — d3-force-3d ships as transitive dep without types
import { forceCollide } from 'd3-force-3d';
import type { GraphData, GraphNode, GraphLink } from './build-force-graph-data';
import type { Device } from '../../types/api';
import { getHealthTier, getStalenessColor, getPulseFrequency } from './health-utils';
import { TIER_COLORS, TYPE_ICON_PATHS, FALLBACK_ICON_PATHS, SELECTION_COLOR } from './constants';
import { useSelectionStore } from '../../stores/selection-store';
import type { ViewportInfo } from './fleet-graph-minimap';

export interface GraphActions {
  navigateTo: (x: number, y: number) => void;
  fitView: () => void;
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
  onViewportChange?: (transform: ViewportInfo) => void;
  graphActionsRef?: React.MutableRefObject<GraphActions | null>;
}

export const FleetGraphCanvas = memo(({
  graphData,
  width,
  height,
  onNodeClick,
  onBackgroundClick,
  onNodeRightClick,
  selectedNodeId,
  onViewportChange,
  graphActionsRef,
}: FleetGraphCanvasProps) => {
  const graphRef = useRef<any>(null);
  const [hoverNode, setHoverNode] = useState<GraphNode | null>(null);
  const highlightNodes = useRef(new Set<GraphNode>());
  const highlightLinks = useRef(new Set<GraphLink>());
  const hasInitialFit = useRef(false);
  const pulseClockRef = useRef(0);
  const canvasWrapperRef = useRef<HTMLDivElement>(null);
  const lassoRef = useRef<{ x1: number; y1: number; x2: number; y2: number } | null>(null);
  const isLassoingRef = useRef(false);
  const [shiftHeld, setShiftHeld] = useState(false);
  const shiftHeldRef = useRef(false);

  // Subscribe to selection store
  const selectedDeviceIds = useSelectionStore((s) => s.selectedDeviceIds);
  const addToSelection = useSelectionStore((s) => s.addToSelection);
  const toggleDevice = useSelectionStore((s) => s.toggleDevice);
  const clearSelection = useSelectionStore((s) => s.clearSelection);

  useEffect(() => {
    let rafId: number;
    const tick = () => {
      pulseClockRef.current = performance.now();
      rafId = requestAnimationFrame(tick);
    };
    rafId = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(rafId);
  }, []);

  // Shift key tracking for lasso mode
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Shift') {
        shiftHeldRef.current = true;
        setShiftHeld(true);
      }
    };
    const onKeyUp = (e: KeyboardEvent) => {
      if (e.key === 'Shift') {
        shiftHeldRef.current = false;
        setShiftHeld(false);
      }
    };
    window.addEventListener('keydown', onKeyDown);
    window.addEventListener('keyup', onKeyUp);
    return () => {
      window.removeEventListener('keydown', onKeyDown);
      window.removeEventListener('keyup', onKeyUp);
    };
  }, []);

  // Suppress browser's native context menu — attach to the wrapper div so it
  // works regardless of when ForceGraph2D creates the inner canvas element
  // (events bubble from canvas → wrapper).
  useEffect(() => {
    const wrapper = canvasWrapperRef.current;
    if (!wrapper) return;

    const suppress = (e: Event) => e.preventDefault();
    wrapper.addEventListener('contextmenu', suppress);
    return () => wrapper.removeEventListener('contextmenu', suppress);
  }, []);

  // Lasso mouse event handlers
  useEffect(() => {
    const fg = graphRef.current;
    const wrapper = canvasWrapperRef.current;
    if (!fg || !wrapper) return;
    const canvasEl = wrapper.querySelector('canvas');
    if (!canvasEl) return;

    const onMouseDown = (e: MouseEvent) => {
      if (!e.shiftKey || e.button !== 0) return;
      const coords = fg.screen2GraphCoords(e.offsetX, e.offsetY);
      lassoRef.current = { x1: coords.x, y1: coords.y, x2: coords.x, y2: coords.y };
      isLassoingRef.current = true;
    };

    const onMouseMove = (e: MouseEvent) => {
      if (!isLassoingRef.current || !lassoRef.current) return;
      const coords = fg.screen2GraphCoords(e.offsetX, e.offsetY);
      lassoRef.current.x2 = coords.x;
      lassoRef.current.y2 = coords.y;
    };

    const onMouseUp = () => {
      if (!isLassoingRef.current || !lassoRef.current) return;
      isLassoingRef.current = false;

      const { x1, y1, x2, y2 } = lassoRef.current;
      const minX = Math.min(x1, x2);
      const maxX = Math.max(x1, x2);
      const minY = Math.min(y1, y2);
      const maxY = Math.max(y1, y2);

      if (maxX - minX > 2 && maxY - minY > 2) {
        const hitIds: string[] = [];
        for (const node of graphData.nodes) {
          if (
            node.type === 'device' &&
            node.x != null &&
            node.y != null &&
            node.x >= minX &&
            node.x <= maxX &&
            node.y >= minY &&
            node.y <= maxY
          ) {
            hitIds.push(node.id);
          }
        }
        if (hitIds.length > 0) {
          addToSelection(hitIds);
        }
      }

      lassoRef.current = null;
    };

    canvasEl.addEventListener('mousedown', onMouseDown);
    canvasEl.addEventListener('mousemove', onMouseMove);
    // Listen on window so releasing the mouse outside the canvas still
    // completes the lasso (prevents stuck lasso state).
    window.addEventListener('mouseup', onMouseUp);
    return () => {
      canvasEl.removeEventListener('mousedown', onMouseDown);
      canvasEl.removeEventListener('mousemove', onMouseMove);
      window.removeEventListener('mouseup', onMouseUp);
    };
  }, [graphData.nodes, addToSelection]);

  // Configure forces after mount
  useEffect(() => {
    if (!graphRef.current) return;
    const fg = graphRef.current;
    fg.d3Force('charge').strength(-200);
    fg.d3Force('link').distance(80);
    fg.d3Force(
      'collide',
      forceCollide((node: GraphNode) => (node.type === 'fleet' ? FLEET_RADIUS + 6 : DEVICE_RADIUS + 4)),
    );
  }, []);

  // Expose graph actions via ref
  useEffect(() => {
    if (!graphActionsRef) return;
    graphActionsRef.current = {
      navigateTo: (x, y) => graphRef.current?.centerAt(x, y, 500),
      fitView: () => graphRef.current?.zoomToFit(400, 60),
    };
  }, [graphActionsRef]);

  // Forward viewport changes to parent (for minimap)
  const handleZoom = useCallback(
    (transform: { k: number; x: number; y: number }) => {
      onViewportChange?.(transform);
    },
    [onViewportChange],
  );

  // Clamp viewport when pan/zoom ends to prevent drifting too far
  const handleZoomEnd = useCallback(
    (transform: { k: number; x: number; y: number }) => {
      if (!hasInitialFit.current) return;
      const fg = graphRef.current;
      if (!fg) return;

      const positioned = graphData.nodes.filter((n) => n.x != null && n.y != null);
      if (positioned.length === 0) return;

      let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
      for (const n of positioned) {
        if (n.x! < minX) minX = n.x!;
        if (n.y! < minY) minY = n.y!;
        if (n.x! > maxX) maxX = n.x!;
        if (n.y! > maxY) maxY = n.y!;
      }

      const rangeX = maxX - minX || 200;
      const rangeY = maxY - minY || 200;
      const padX = Math.max(rangeX * 1.5, 500);
      const padY = Math.max(rangeY * 1.5, 500);
      const boundsCenterX = (minX + maxX) / 2;
      const boundsCenterY = (minY + maxY) / 2;

      const centerWorldX = (width / 2 - transform.x) / transform.k;
      const centerWorldY = (height / 2 - transform.y) / transform.k;

      const clampedX = Math.max(boundsCenterX - padX, Math.min(boundsCenterX + padX, centerWorldX));
      const clampedY = Math.max(boundsCenterY - padY, Math.min(boundsCenterY + padY, centerWorldY));

      if (clampedX !== centerWorldX || clampedY !== centerWorldY) {
        fg.centerAt(clampedX, clampedY, 300);
      }
    },
    [graphData.nodes, width, height],
  );

  // Center on selected node
  useEffect(() => {
    if (!selectedNodeId || !graphRef.current) return;
    const node = graphData.nodes.find((n) => n.id === selectedNodeId);
    if (node?.x != null && node?.y != null) {
      graphRef.current.centerAt(node.x, node.y, 500);
      graphRef.current.zoom(2, 500);
    }
  }, [selectedNodeId, graphData.nodes]);

  // Fit to view only on initial simulation settle
  const handleEngineStop = useCallback(() => {
    if (!hasInitialFit.current) {
      hasInitialFit.current = true;
      graphRef.current?.zoomToFit(400, 60);
    }
  }, []);

  // Hover handler — update highlight sets
  const handleNodeHover = useCallback((node: GraphNode | null) => {
    highlightNodes.current.clear();
    highlightLinks.current.clear();

    if (node) {
      highlightNodes.current.add(node);
      node.neighbors.forEach((n) => highlightNodes.current.add(n));
      node.links.forEach((l) => highlightLinks.current.add(l));
    }

    setHoverNode(node);
  }, []);

  // Click handler
  const handleNodeClick = useCallback(
    (node: GraphNode, event: MouseEvent) => {
      if (event.shiftKey && node.type === 'device') {
        toggleDevice(node.id);
        return;
      }
      if (node.type === 'device' && node.device) {
        onNodeClick(node.device, { x: event.clientX, y: event.clientY });
      }
    },
    [onNodeClick, toggleDevice],
  );

  // Background click: clear selection unless Shift is held
  const handleBackgroundClickInternal = useCallback(
    (event: MouseEvent) => {
      if (!event.shiftKey) {
        clearSelection();
      }
      onBackgroundClick(event);
    },
    [onBackgroundClick, clearSelection],
  );

  // --- Canvas rendering callbacks ---
  const paintNode = useCallback(
    (node: GraphNode, ctx: CanvasRenderingContext2D, globalScale: number) => {
      const isFleet = node.type === 'fleet';
      const baseRadius = isFleet ? FLEET_RADIUS : DEVICE_RADIUS;
      const isHighlighted = highlightNodes.current.has(node);
      const isHovered = node === hoverNode;
      const shouldDim = hoverNode && !isHighlighted;

      if (node.x == null || node.y == null) return;

      // Opacity
      ctx.globalAlpha = shouldDim ? DIM_OPACITY : 1;

      // Scale on hover
      const radius = isHovered ? baseRadius * HOVER_SCALE : baseRadius;

      // Glow effect for hovered/highlighted nodes (device nodes only)
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
        // --- Fleet node: flat rectangle sized to text with integrated health bar ---
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

        // Flat rect background (no rounded corners)
        ctx.fillStyle = node.color;
        ctx.fillRect(rx, ry, rectW, rectH);

        // Reset shadow before bar and text
        ctx.shadowBlur = 0;

        // Health bar integrated at the bottom of the rect
        if (node.tierRatios) {
          const scaledBarH = barH * hoverScale;
          const barY = ry + rectH - scaledBarH;
          const segments: [number, string][] = [
            [node.tierRatios.fresh, TIER_COLORS.fresh],
            [node.tierRatios.warm, TIER_COLORS.warm],
            [node.tierRatios.stale, TIER_COLORS.stale],
            [node.tierRatios.dead, TIER_COLORS.dead],
          ];
          let offsetX = 0;
          for (const [ratio, color] of segments) {
            if (ratio <= 0) continue;
            const segW = ratio * rectW;
            ctx.fillStyle = color;
            ctx.fillRect(rx + offsetX, barY, segW, scaledBarH);
            offsetX += segW;
          }

          // Separator line between name area and health bar
          ctx.strokeStyle = 'rgba(0,0,0,0.6)';
          ctx.lineWidth = 0.5;
          ctx.beginPath();
          ctx.moveTo(rx, barY);
          ctx.lineTo(rx + rectW, barY);
          ctx.stroke();
        }

        // Outer border drawn last so it sits on top of health segments
        ctx.strokeStyle = 'rgba(0,0,0,0.6)';
        ctx.lineWidth = 0.5;
        ctx.strokeRect(rx, ry, rectW, rectH);

        // Fleet name — shifted up by half the bar height to center in the name area
        ctx.font = FLEET_LABEL_FONT;
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        ctx.fillStyle = '#ffffff';
        ctx.fillText(node.name, node.x!, node.y! - (barH * hoverScale) / 2);

        // Device count below rect
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
        // Reset shadow before drawing device node — shadow glow is applied
        // via the selection ring below, not the main circle.
        ctx.shadowBlur = 0;
        // --- Device node: staleness-based color + pulse + uptime ring ---
        const now = Date.now();
        const stalenessMs = node.lastSeenTimestamp ? now - node.lastSeenTimestamp : NaN;
        const tier = getHealthTier(stalenessMs, node.status);
        const stalenessColor = getStalenessColor(stalenessMs, node.status);
        const pulseHz = getPulseFrequency(tier);

        // Dead nodes shrink slightly
        const effectiveRadius = tier === 'dead' ? radius * 0.85 : radius;

        // Pulse glow (radial gradient behind node) — only for fresh/warm
        if (pulseHz > 0) {
          const t = pulseClockRef.current / 1000;
          const glowRadius = effectiveRadius + 4 + 4 * Math.sin(2 * Math.PI * pulseHz * t);
          const gradient = ctx.createRadialGradient(
            node.x!, node.y!, effectiveRadius,
            node.x!, node.y!, glowRadius,
          );
          // Use rgba() instead of 8-digit hex — Canvas 2D spec mandates CSS
          // Color Level 3 parsing, which doesn't include #RRGGBBAA. Safari's
          // canvas silently treats 8-digit hex as transparent.
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

        // Main circle
        ctx.beginPath();
        ctx.arc(node.x!, node.y!, effectiveRadius, 0, 2 * Math.PI);
        ctx.fillStyle = stalenessColor;
        ctx.fill();

        // Uptime ring
        if (node.uptimeArcAngle && node.uptimeArcAngle > 0) {
          ctx.beginPath();
          const startAngle = -Math.PI / 2; // 12 o'clock
          ctx.arc(node.x!, node.y!, effectiveRadius + 3, startAngle, startAngle + node.uptimeArcAngle);
          ctx.strokeStyle = stalenessColor;
          ctx.lineWidth = 2;
          ctx.stroke();
        }

        // Selection ring (drawn after main circle and uptime ring so glow is visible)
        if (selectedDeviceIds.has(node.id)) {
          ctx.beginPath();
          ctx.arc(node.x!, node.y!, effectiveRadius + 7, 0, 2 * Math.PI);
          ctx.strokeStyle = SELECTION_COLOR;
          ctx.lineWidth = 2;
          ctx.shadowColor = SELECTION_COLOR;
          ctx.shadowBlur = 8;
          ctx.stroke();
          ctx.shadowBlur = 0;
        }

        // Device type icon inside circle
        const iconPaths = getIconPaths(node.deviceTypeName);
        const iconSize = effectiveRadius * 1.2;
        ctx.fillStyle = '#ffffff';
        drawIcon(ctx, iconPaths, node.x!, node.y!, iconSize);

        // Name label below
        const fontSize = Math.max(10, 12 / globalScale);
        ctx.font = `${fontSize}px -apple-system, sans-serif`;
        ctx.fillStyle = shouldDim ? `rgba(255,255,255,${DIM_OPACITY})` : 'rgba(255,255,255,0.8)';
        ctx.fillText(node.name, node.x!, node.y! + effectiveRadius + fontSize + 2);
      }

      // Reset
      ctx.globalAlpha = 1;
    },
    [hoverNode, selectedDeviceIds],
  );

  const paintLink = useCallback(
    (link: GraphLink, ctx: CanvasRenderingContext2D) => {
      const isHighlighted = highlightLinks.current.has(link);
      const shouldDim = hoverNode && !isHighlighted;

      // D3 mutates source/target to objects
      const source = link.source as any as GraphNode;
      const target = link.target as any as GraphNode;
      if (source.x == null || target.x == null) return;

      // Determine if the device end of the link is active
      const deviceNode = source.type === 'device' ? source : target.type === 'device' ? target : null;
      const isActive = deviceNode?.status === 'online' || deviceNode?.status === 'warning';

      ctx.beginPath();
      if (isActive) {
        ctx.setLineDash([4, 4]);
        // Animate dash offset so the dashes appear to flow
        ctx.lineDashOffset = -(pulseClockRef.current / 1000) * 12;
      } else {
        ctx.setLineDash([3, 5]);
      }
      ctx.moveTo(source.x!, source.y!);
      ctx.lineTo(target.x!, target.y!);

      if (isHighlighted) {
        ctx.strokeStyle = isActive ? 'rgba(0, 200, 80, 0.9)' : 'rgba(255, 60, 60, 0.8)';
        ctx.lineWidth = 1.5;
        ctx.shadowColor = isActive ? 'rgba(0, 200, 80, 0.4)' : 'rgba(255, 60, 60, 0.3)';
        ctx.shadowBlur = 6;
      } else if (shouldDim) {
        ctx.strokeStyle = isActive
          ? `rgba(0, 200, 80, ${DIM_OPACITY * 0.5})`
          : `rgba(255, 60, 60, ${DIM_OPACITY * 0.5})`;
        ctx.lineWidth = 0.5;
        ctx.shadowBlur = 0;
      } else {
        ctx.strokeStyle = isActive ? 'rgba(0, 200, 80, 0.6)' : 'rgba(255, 60, 60, 0.45)';
        ctx.lineWidth = isActive ? 1 : 0.5;
        ctx.shadowBlur = 0;
      }

      ctx.stroke();
      ctx.setLineDash([]);
      ctx.lineDashOffset = 0;
      ctx.shadowBlur = 0;
    },
    [hoverNode],
  );

  // Draw a grid in world-space (onRenderFramePre context is already transformed)
  const paintGrid = useCallback((ctx: CanvasRenderingContext2D, globalScale: number) => {
    const fg = graphRef.current;
    if (!fg) return;

    // Get visible world-space bounds
    const topLeft = fg.screen2GraphCoords(0, 0);
    const bottomRight = fg.screen2GraphCoords(width, height);

    const step = GRID_SIZE;
    const startX = Math.floor(topLeft.x / step) * step;
    const startY = Math.floor(topLeft.y / step) * step;
    const endX = Math.ceil(bottomRight.x / step) * step;
    const endY = Math.ceil(bottomRight.y / step) * step;

    // Draw in world-space — context already has the zoom/pan transform
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
  }, [width, height]);

  // Disable pan when Shift is held (lasso mode)
  const enablePanInteraction = useCallback((ev: MouseEvent) => !ev.shiftKey, []);

  // Draw lasso selection rectangle overlay
  const paintLasso = useCallback((ctx: CanvasRenderingContext2D, globalScale: number) => {
    if (!lassoRef.current) return;
    const { x1, y1, x2, y2 } = lassoRef.current;
    const x = Math.min(x1, x2);
    const y = Math.min(y1, y2);
    const w = Math.abs(x2 - x1);
    const h = Math.abs(y2 - y1);

    ctx.save();
    ctx.fillStyle = 'rgba(45, 114, 210, 0.15)';
    ctx.fillRect(x, y, w, h);
    ctx.strokeStyle = SELECTION_COLOR;
    ctx.lineWidth = 1 / globalScale;
    ctx.setLineDash([4 / globalScale, 4 / globalScale]);
    ctx.strokeRect(x, y, w, h);
    ctx.setLineDash([]);
    ctx.restore();
  }, []);

  return (
    <div ref={canvasWrapperRef} style={{ width, height }}>
      <ForceGraph2D
        ref={graphRef}
        graphData={graphData}
        width={width}
        height={height}
        backgroundColor="#171717"
        onRenderFramePre={paintGrid as any}
        onRenderFramePost={paintLasso as any}
        nodeCanvasObject={paintNode as any}
        nodeCanvasObjectMode={() => 'replace'}
        linkCanvasObject={paintLink as any}
        linkCanvasObjectMode={() => 'replace'}
        onNodeHover={handleNodeHover as any}
        onNodeClick={handleNodeClick as any}
        onNodeRightClick={onNodeRightClick as any}
        onBackgroundClick={handleBackgroundClickInternal as any}
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
        onZoomEnd={handleZoomEnd as any}
        minZoom={0.5}
        maxZoom={8}
      />
    </div>
  );
});
