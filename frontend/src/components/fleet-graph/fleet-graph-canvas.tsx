import { useCallback, useEffect, useRef, useState } from 'react';
import ForceGraph2D from 'react-force-graph-2d';
// @ts-ignore — d3-force-3d ships as transitive dep without types
import { forceCollide } from 'd3-force-3d';
import type { GraphData, GraphNode, GraphLink } from './build-force-graph-data';
import type { Device } from '../../types/api';

// --- Constants ---
const FLEET_RADIUS = 18;
const DEVICE_RADIUS = 8;
const HOVER_SCALE = 1.3;
const DIM_OPACITY = 0.15;
const FLEET_LABEL_FONT = 'bold 12px -apple-system, BlinkMacSystemFont, sans-serif';

interface FleetGraphCanvasProps {
  graphData: GraphData;
  width: number;
  height: number;
  onNodeClick: (device: Device, position: { x: number; y: number }) => void;
  onBackgroundClick: () => void;
}

export const FleetGraphCanvas = ({
  graphData,
  width,
  height,
  onNodeClick,
  onBackgroundClick,
}: FleetGraphCanvasProps) => {
  const graphRef = useRef<any>(null);
  const [hoverNode, setHoverNode] = useState<GraphNode | null>(null);
  const highlightNodes = useRef(new Set<GraphNode>());
  const highlightLinks = useRef(new Set<GraphLink>());

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

  // Fit to view when simulation settles
  const handleEngineStop = useCallback(() => {
    graphRef.current?.zoomToFit(400, 60);
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
          ctx.fillText(`${node.deviceCount} devices`, node.x!, node.y! + FLEET_RADIUS + 12);
        }
      } else {
        // Device: type abbreviation inside circle
        ctx.font = `bold ${Math.max(8, radius)}px -apple-system, sans-serif`;
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        ctx.fillStyle = '#ffffff';
        ctx.fillText(node.typeAbbrev ?? '?', node.x!, node.y!);

        // Name label below
        const fontSize = Math.max(10, 12 / globalScale);
        ctx.font = `${fontSize}px -apple-system, sans-serif`;
        ctx.fillStyle = shouldDim ? `rgba(255,255,255,${DIM_OPACITY})` : 'rgba(255,255,255,0.8)';
        ctx.fillText(node.name, node.x!, node.y! + radius + fontSize + 2);
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

  return (
    <ForceGraph2D
      ref={graphRef}
      graphData={graphData}
      width={width}
      height={height}
      backgroundColor="#000000"
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
