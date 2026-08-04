import { useEffect, useRef, useCallback } from 'react';
import type { GraphNode, GraphLink } from './build-force-graph-data';
import { getStalenessColor } from './health-utils';

export interface ViewportInfo {
  k: number;
  x: number;
  y: number;
}

interface FleetGraphMinimapProps {
  nodes: GraphNode[];
  links: GraphLink[];
  viewportRef: React.RefObject<ViewportInfo | null>;
  canvasWidth: number;
  canvasHeight: number;
  onNavigate: (x: number, y: number, durationMs?: number) => void;
  /** Parent writes a redraw callback here so it can trigger imperative redraws */
  drawRef: React.MutableRefObject<(() => void) | null>;
}

const MINIMAP_HEIGHT = 100;
const PAD = 10;

function computeWorldBounds(nodes: GraphNode[]) {
  const positioned = nodes.filter((n) => n.x != null && n.y != null);
  if (positioned.length === 0) return null;

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

  const rangeX = maxX - minX || 1;
  const rangeY = maxY - minY || 1;
  const margin = Math.max(rangeX, rangeY) * 0.15;
  return { minX: minX - margin, minY: minY - margin, maxX: maxX + margin, maxY: maxY + margin };
}

function computeMinimapProjection(nodes: GraphNode[], width: number, height: number) {
  const bounds = computeWorldBounds(nodes);
  if (!bounds) return null;

  let { minX, minY, maxX, maxY } = bounds;
  const availW = width - PAD * 2;
  const availH = height - PAD * 2;
  if (availW <= 0 || availH <= 0) return null;

  // Expand world bounds to match the minimap aspect ratio so the content fills
  // the entire minimap with no empty strips.
  const worldW = maxX - minX;
  const worldH = maxY - minY;
  const worldAspect = worldW / worldH;
  const minimapAspect = availW / availH;

  if (worldAspect < minimapAspect) {
    const targetW = worldH * minimapAspect;
    const cx = (minX + maxX) / 2;
    minX = cx - targetW / 2;
    maxX = cx + targetW / 2;
  } else {
    const targetH = worldW / minimapAspect;
    const cy = (minY + maxY) / 2;
    minY = cy - targetH / 2;
    maxY = cy + targetH / 2;
  }

  const finalW = maxX - minX;
  const finalH = maxY - minY;
  const scale = Math.min(availW / finalW, availH / finalH);
  const offsetX = (width - finalW * scale) / 2;
  const offsetY = (height - finalH * scale) / 2;

  const toMX = (wx: number) => (wx - minX) * scale + offsetX;
  const toMY = (wy: number) => (wy - minY) * scale + offsetY;
  const toWX = (mx: number) => (mx - offsetX) / scale + minX;
  const toWY = (my: number) => (my - offsetY) / scale + minY;
  const clampMX = (mx: number) => Math.max(offsetX, Math.min(offsetX + finalW * scale, mx));
  const clampMY = (my: number) => Math.max(offsetY, Math.min(offsetY + finalH * scale, my));

  return { scale, toMX, toMY, toWX, toWY, clampMX, clampMY };
}

export const FleetGraphMinimap = ({
  nodes,
  links,
  viewportRef,
  canvasWidth,
  canvasHeight,
  onNavigate,
  drawRef,
}: FleetGraphMinimapProps) => {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const dragOffsetRef = useRef<{ x: number; y: number } | null>(null);

  const draw = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const viewport = viewportRef.current;

    const dpr = window.devicePixelRatio || 1;
    const rect = canvas.getBoundingClientRect();
    const w = rect.width;
    const h = rect.height;
    if (w === 0 || h === 0) return;

    canvas.width = w * dpr;
    canvas.height = h * dpr;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    ctx.scale(dpr, dpr);

    // Background — match the main graph canvas (#171717)
    ctx.fillStyle = '#171717';
    ctx.beginPath();
    ctx.roundRect(0, 0, w, h, 4);
    ctx.fill();

    const projection = computeMinimapProjection(nodes, w, h);
    if (!projection) return;
    const { scale, toMX, toMY } = projection;

    // Links (drawn before nodes so nodes appear on top)
    ctx.lineWidth = 1.5;
    for (const link of links) {
      const src = typeof link.source === 'string' ? null : link.source;
      const tgt = typeof link.target === 'string' ? null : link.target;
      if (src?.x == null || src.y == null || tgt?.x == null || tgt.y == null) continue;

      ctx.beginPath();
      ctx.moveTo(toMX(src.x), toMY(src.y));
      ctx.lineTo(toMX(tgt.x), toMY(tgt.y));
      ctx.strokeStyle = 'rgba(255, 255, 255, 0.12)';
      ctx.stroke();
    }

    // Nodes
    const positioned = nodes.filter((n) => n.x != null && n.y != null);
    for (const node of positioned) {
      const mx = toMX(node.x!);
      const my = toMY(node.y!);

      if (node.type === 'fleet') {
        ctx.fillStyle = node.color || '#4a90d9';
        ctx.fillRect(mx - 2, my - 2, 4, 4);
      } else {
        const now = Date.now();
        const stalenessMs = node.lastSeenTimestamp ? now - node.lastSeenTimestamp : NaN;
        const color = getStalenessColor(stalenessMs, node.status);
        ctx.beginPath();
        ctx.arc(mx, my, 1.5, 0, 2 * Math.PI);
        ctx.fillStyle = color;
        ctx.fill();
      }
    }

    // Viewport rectangle is the top-most minimap layer so it stays legible
    // over dense clusters of links and nodes.
    // force-graph's onZoom passes { k, x, y } where x,y are the world-space
    // CENTER of the viewport (not d3-zoom screen-space translation).
    if (viewport && canvasWidth > 0 && canvasHeight > 0) {
      const halfW = canvasWidth / viewport.k / 2;
      const halfH = canvasHeight / viewport.k / 2;
      const vMinX = viewport.x - halfW;
      const vMinY = viewport.y - halfH;
      const vMaxX = viewport.x + halfW;
      const vMaxY = viewport.y + halfH;

      const rx = toMX(vMinX);
      const ry = toMY(vMinY);
      const rw = (vMaxX - vMinX) * scale;
      const rh = (vMaxY - vMinY) * scale;

      ctx.fillStyle = 'rgba(255, 255, 255, 0.04)';
      ctx.fillRect(rx, ry, rw, rh);
      ctx.strokeStyle = 'rgba(255, 255, 255, 0.4)';
      ctx.lineWidth = 1;
      ctx.strokeRect(rx, ry, rw, rh);
    }
  }, [nodes, links, viewportRef, canvasWidth, canvasHeight]);

  const getPointerWorldPosition = useCallback(
    (event: React.PointerEvent<HTMLCanvasElement>) => {
      const canvas = canvasRef.current;
      if (!canvas) return null;

      const rect = canvas.getBoundingClientRect();
      const projection = computeMinimapProjection(nodes, rect.width, rect.height);
      if (!projection) return null;

      const mx = projection.clampMX(event.clientX - rect.left);
      const my = projection.clampMY(event.clientY - rect.top);
      return { x: projection.toWX(mx), y: projection.toWY(my), mx, my, projection };
    },
    [nodes],
  );

  const getViewportRect = useCallback(
    (projection: NonNullable<ReturnType<typeof computeMinimapProjection>>) => {
      const viewport = viewportRef.current;
      if (!viewport || canvasWidth <= 0 || canvasHeight <= 0) return null;

      const halfW = canvasWidth / viewport.k / 2;
      const halfH = canvasHeight / viewport.k / 2;
      const x = projection.toMX(viewport.x - halfW);
      const y = projection.toMY(viewport.y - halfH);

      return {
        x,
        y,
        width: halfW * 2 * projection.scale,
        height: halfH * 2 * projection.scale,
      };
    },
    [canvasHeight, canvasWidth, viewportRef],
  );

  const navigateFromPointer = useCallback(
    (event: React.PointerEvent<HTMLCanvasElement>, durationMs: number) => {
      const pointer = getPointerWorldPosition(event);
      if (!pointer) return;

      const dragOffset = dragOffsetRef.current;
      onNavigate(pointer.x + (dragOffset?.x ?? 0), pointer.y + (dragOffset?.y ?? 0), durationMs);
    },
    [getPointerWorldPosition, onNavigate],
  );

  const handlePointerDown = useCallback(
    (event: React.PointerEvent<HTMLCanvasElement>) => {
      if (event.button !== 0) return;
      const pointer = getPointerWorldPosition(event);
      if (!pointer) return;

      const viewport = viewportRef.current;
      const viewportRect = getViewportRect(pointer.projection);
      const isInsideViewport =
        viewportRect &&
        pointer.mx >= viewportRect.x &&
        pointer.mx <= viewportRect.x + viewportRect.width &&
        pointer.my >= viewportRect.y &&
        pointer.my <= viewportRect.y + viewportRect.height;

      dragOffsetRef.current =
        isInsideViewport && viewport
          ? { x: viewport.x - pointer.x, y: viewport.y - pointer.y }
          : { x: 0, y: 0 };

      event.currentTarget.setPointerCapture(event.pointerId);
      navigateFromPointer(event, isInsideViewport ? 0 : 180);
    },
    [getPointerWorldPosition, getViewportRect, navigateFromPointer, viewportRef],
  );

  const handlePointerMove = useCallback(
    (event: React.PointerEvent<HTMLCanvasElement>) => {
      if (!dragOffsetRef.current) return;
      navigateFromPointer(event, 0);
    },
    [navigateFromPointer],
  );

  const handlePointerUp = useCallback((event: React.PointerEvent<HTMLCanvasElement>) => {
    dragOffsetRef.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
  }, []);

  // Register draw function so parent can call it imperatively on viewport changes
  useEffect(() => {
    drawRef.current = draw;
    return () => {
      drawRef.current = null;
    };
  }, [draw, drawRef]);

  // Draw on mount and when nodes/canvas size change
  useEffect(() => {
    draw();
  }, [draw]);

  return (
    <canvas
      ref={canvasRef}
      className="fleet-graph-minimap"
      aria-label="Fleet graph minimap"
      role="application"
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUp}
      onPointerCancel={handlePointerUp}
      style={{ width: '100%', height: MINIMAP_HEIGHT }}
    />
  );
};
