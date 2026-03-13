import { useCallback, useEffect, useRef, useState } from 'react';
import ForceGraph2D from 'react-force-graph-2d';
// @ts-ignore — d3-force-3d ships as transitive dep without types
import { forceCollide } from 'd3-force-3d';
import type { GraphData, GraphNode, GraphLink } from './build-force-graph-data';
import type { Device } from '../../types/api';
import { getHealthTier, getStalenessColor, getPulseFrequency } from './health-utils';
import { TIER_COLORS, TYPE_ICON_PATHS, FALLBACK_ICON_PATHS } from './constants';

// --- Constants ---
const FLEET_RADIUS = 18;
const DEVICE_RADIUS = 8;
const HOVER_SCALE = 1.3;
const DIM_OPACITY = 0.15;
const FLEET_LABEL_FONT = 'bold 12px -apple-system, BlinkMacSystemFont, sans-serif';
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
  onBackgroundClick: () => void;
  selectedNodeId?: string | null;
}

export const FleetGraphCanvas = ({
  graphData,
  width,
  height,
  onNodeClick,
  onBackgroundClick,
  selectedNodeId,
}: FleetGraphCanvasProps) => {
  const graphRef = useRef<any>(null);
  const [hoverNode, setHoverNode] = useState<GraphNode | null>(null);
  const highlightNodes = useRef(new Set<GraphNode>());
  const highlightLinks = useRef(new Set<GraphLink>());
  const hasInitialFit = useRef(false);
  const pulseClockRef = useRef(0);

  useEffect(() => {
    let rafId: number;
    const tick = () => {
      pulseClockRef.current = performance.now();
      rafId = requestAnimationFrame(tick);
    };
    rafId = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(rafId);
  }, []);

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
      if (node.type === 'device' && node.device) {
        onNodeClick(node.device, { x: event.clientX, y: event.clientY });
      }
    },
    [onNodeClick],
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

      // Glow effect for hovered/highlighted nodes
      if (isHovered) {
        ctx.shadowColor = node.color;
        ctx.shadowBlur = 20;
      } else if (isHighlighted) {
        ctx.shadowColor = node.color;
        ctx.shadowBlur = 10;
      } else {
        ctx.shadowBlur = 0;
      }

      // Draw circle
      ctx.beginPath();
      ctx.arc(node.x!, node.y!, radius, 0, 2 * Math.PI);
      ctx.fillStyle = node.color;
      ctx.fill();

      // Reset shadow for text
      ctx.shadowBlur = 0;

      if (isFleet) {
        // Fleet: name label centered
        ctx.font = FLEET_LABEL_FONT;
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        ctx.fillStyle = '#ffffff';
        ctx.fillText(node.name, node.x!, node.y!);

        // Device count below
        if (node.deviceCount != null) {
          ctx.font = '10px -apple-system, sans-serif';
          ctx.fillStyle = 'rgba(255,255,255,0.6)';
          ctx.fillText(`${node.deviceCount} device${node.deviceCount === 1 ? '' : 's'}`, node.x!, node.y! + FLEET_RADIUS + 12);
        }

        // Fleet hub aggregate health donut ring
        if (node.tierRatios) {
          const ringRadius = radius + 4;
          const ringWidth = 3;
          let angle = -Math.PI / 2; // start at 12 o'clock

          const segments: [number, string][] = [
            [node.tierRatios.fresh, TIER_COLORS.fresh],
            [node.tierRatios.warm, TIER_COLORS.warm],
            [node.tierRatios.stale, TIER_COLORS.stale],
            [node.tierRatios.dead, TIER_COLORS.dead],
          ];

          for (const [ratio, color] of segments) {
            if (ratio <= 0) continue;
            const arcLen = ratio * 2 * Math.PI;
            ctx.beginPath();
            ctx.arc(node.x!, node.y!, ringRadius, angle, angle + arcLen);
            ctx.strokeStyle = color;
            ctx.lineWidth = ringWidth;
            ctx.stroke();
            angle += arcLen;
          }
        }
      } else {
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
          gradient.addColorStop(0, stalenessColor + '80'); // 50% alpha
          gradient.addColorStop(1, stalenessColor + '00'); // fully transparent
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
    [hoverNode],
  );

  const paintLink = useCallback(
    (link: GraphLink, ctx: CanvasRenderingContext2D) => {
      const isHighlighted = highlightLinks.current.has(link);
      const shouldDim = hoverNode && !isHighlighted;

      // D3 mutates source/target to objects
      const source = link.source as any;
      const target = link.target as any;
      if (source.x == null || target.x == null) return;

      ctx.beginPath();
      ctx.moveTo(source.x, source.y);
      ctx.lineTo(target.x, target.y);

      if (isHighlighted) {
        ctx.strokeStyle = 'rgba(255, 255, 255, 0.6)';
        ctx.lineWidth = 1.5;
        ctx.shadowColor = 'rgba(255, 255, 255, 0.3)';
        ctx.shadowBlur = 6;
      } else {
        ctx.strokeStyle = shouldDim
          ? `rgba(255, 255, 255, ${DIM_OPACITY * 0.5})`
          : 'rgba(255, 255, 255, 0.15)';
        ctx.lineWidth = 0.5;
        ctx.shadowBlur = 0;
      }

      ctx.stroke();
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

  return (
    <ForceGraph2D
      ref={graphRef}
      graphData={graphData}
      width={width}
      height={height}
      backgroundColor="#000000"
      onRenderFramePre={paintGrid as any}
      nodeCanvasObject={paintNode as any}
      nodeCanvasObjectMode={() => 'replace'}
      linkCanvasObject={paintLink as any}
      linkCanvasObjectMode={() => 'replace'}
      onNodeHover={handleNodeHover as any}
      onNodeClick={handleNodeClick as any}
      onBackgroundClick={onBackgroundClick}
      onEngineStop={handleEngineStop}
      nodeVal="val"
      d3AlphaDecay={0.02}
      d3VelocityDecay={0.3}
      warmupTicks={50}
      cooldownTicks={200}
      autoPauseRedraw={false}
      nodeLabel=""
    />
  );
};
