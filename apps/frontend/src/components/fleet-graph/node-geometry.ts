import type { GraphNode } from './build-force-graph-data';
export const FLEET_LABEL_FONT = 'bold 10px -apple-system, BlinkMacSystemFont, sans-serif';
const FLEET_RADIUS = 14;
const DEVICE_RADIUS = 11;
const EXTERNAL_RADIUS = 8;
const HOVER_SCALE = 1.3;
export function nodeRadius(node: GraphNode, hovered: boolean): number {
  const base =
    node.type === 'fleet'
      ? FLEET_RADIUS
      : node.type === 'external'
        ? EXTERNAL_RADIUS
        : DEVICE_RADIUS;
  return base * (hovered ? HOVER_SCALE : 1);
}
export function fleetGeometry(textWidth: number, hovered: boolean) {
  const scale = hovered ? 1.1 : 1;
  return {
    width: (textWidth + 16) * scale,
    height: (FLEET_RADIUS + 4 + 3) * scale,
    barHeight: 3 * scale,
  };
}
export function externalNodeSide(radius: number) {
  return radius * 2.35;
}

export function paintNodeHitArea(node: GraphNode, color: string, ctx: CanvasRenderingContext2D) {
  if (node.x == null || node.y == null) return;
  ctx.fillStyle = color;
  if (node.type === 'fleet') {
    ctx.font = FLEET_LABEL_FONT;
    const { width, height } = fleetGeometry(ctx.measureText(node.name).width, true);
    ctx.fillRect(node.x - width / 2, node.y - height / 2, width, height);
  } else {
    const radius = nodeRadius(node, true);
    const side = node.type === 'external' ? externalNodeSide(radius) : radius * 2;
    ctx.fillRect(node.x - side / 2, node.y - side / 2, side, side);
  }
}
