import { useCallback, useEffect, useRef } from 'react';
import type { GraphNode } from './build-force-graph-data';
import { getStalenessColor } from './health-utils';

export interface ViewportInfo {
  k: number;
  x: number;
  y: number;
}

interface FleetGraphMinimapProps {
  nodes: GraphNode[];
  viewportRef: React.RefObject<ViewportInfo | null>;
  canvasWidth: number;
  canvasHeight: number;
  onNavigate: (worldX: number, worldY: number) => void;
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

export const FleetGraphMinimap = ({
  nodes,
  viewportRef,
  canvasWidth,
  canvasHeight,
  onNavigate,
  drawRef,
}: FleetGraphMinimapProps) => {
  const canvasRef = useRef<HTMLCanvasElement>(null);

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

    // Background
    ctx.fillStyle = 'rgba(0, 0, 0, 0.5)';
    ctx.beginPath();
    ctx.roundRect(0, 0, w, h, 4);
    ctx.fill();

    const bounds = computeWorldBounds(nodes);
    if (!bounds) return;

    const { minX, minY, maxX, maxY } = bounds;
    const worldW = maxX - minX;
    const worldH = maxY - minY;

    const scale = Math.min((w - PAD * 2) / worldW, (h - PAD * 2) / worldH);
    const offsetX = (w - worldW * scale) / 2;
    const offsetY = (h - worldH * scale) / 2;

    const toMX = (wx: number) => (wx - minX) * scale + offsetX;
    const toMY = (wy: number) => (wy - minY) * scale + offsetY;

    // Viewport rectangle (drawn behind nodes)
    if (viewport && canvasWidth > 0 && canvasHeight > 0) {
      const vMinX = (0 - viewport.x) / viewport.k;
      const vMinY = (0 - viewport.y) / viewport.k;
      const vMaxX = (canvasWidth - viewport.x) / viewport.k;
      const vMaxY = (canvasHeight - viewport.y) / viewport.k;

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
  }, [nodes, viewportRef, canvasWidth, canvasHeight]);

  // Register draw function so parent can call it imperatively on viewport changes
  useEffect(() => {
    drawRef.current = draw;
    return () => { drawRef.current = null; };
  }, [draw, drawRef]);

  // Draw on mount and when nodes/canvas size change
  useEffect(() => {
    draw();
  }, [draw]);

  const handleMouseDown = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      e.stopPropagation();
      e.preventDefault();

      const canvas = canvasRef.current;
      if (!canvas) return;
      const rect = canvas.getBoundingClientRect();
      const mx = e.clientX - rect.left;
      const my = e.clientY - rect.top;

      const bounds = computeWorldBounds(nodes);
      if (!bounds) return;

      const { minX, minY, maxX, maxY } = bounds;
      const worldW = maxX - minX;
      const worldH = maxY - minY;
      const w = rect.width;
      const h = rect.height;

      const scale = Math.min((w - PAD * 2) / worldW, (h - PAD * 2) / worldH);
      const offsetX = (w - worldW * scale) / 2;
      const offsetY = (h - worldH * scale) / 2;

      const worldX = (mx - offsetX) / scale + minX;
      const worldY = (my - offsetY) / scale + minY;

      onNavigate(worldX, worldY);
    },
    [nodes, onNavigate],
  );

  return (
    <canvas
      ref={canvasRef}
      className="fleet-graph-minimap"
      style={{ width: '100%', height: MINIMAP_HEIGHT, cursor: 'crosshair' }}
      onMouseDown={handleMouseDown}
    />
  );
};
