import type { GraphNode, GraphLink } from './build-force-graph-data';
import { getHealthTier, getStalenessColor, getPulseFrequency } from './health-utils';
import { TIER_COLORS, SELECTION_COLOR } from './constants';
import { getIconPaths } from './graph-icons';
import { nodeRadius, fleetGeometry, externalNodeSide, FLEET_LABEL_FONT } from './node-geometry';
export type AlertSeverity = 'info' | 'warning' | 'critical';

export interface DeviceAlertBadge {
  count: number;
  severity: AlertSeverity;
}

const DIM_OPACITY = 0.15;
const ALERT_BADGE_COLORS: Record<AlertSeverity, string> = {
  info: '#2D72D2',
  warning: '#D9822B',
  critical: '#C23030',
};

function colorWithAlpha(color: string | undefined, alpha: number): string {
  const match = /^#?([0-9a-f]{6})$/i.exec(color ?? '');
  const hex = match?.[1];
  if (!hex) return `rgba(123,139,154,${alpha})`;

  const r = parseInt(hex.slice(0, 2), 16);
  const g = parseInt(hex.slice(2, 4), 16);
  const b = parseInt(hex.slice(4, 6), 16);
  return `rgba(${r},${g},${b},${alpha})`;
}

function drawRoundedRect(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  width: number,
  height: number,
  radius: number,
) {
  const r = Math.min(radius, width / 2, height / 2);
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.lineTo(x + width - r, y);
  ctx.quadraticCurveTo(x + width, y, x + width, y + r);
  ctx.lineTo(x + width, y + height - r);
  ctx.quadraticCurveTo(x + width, y + height, x + width - r, y + height);
  ctx.lineTo(x + r, y + height);
  ctx.quadraticCurveTo(x, y + height, x, y + height - r);
  ctx.lineTo(x, y + r);
  ctx.quadraticCurveTo(x, y, x + r, y);
  ctx.closePath();
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

export interface GraphRenderState {
  activeHoverNode: GraphNode | null;
  selectedNodeId?: string | null;
  hoverHighlight: { nodes: Set<GraphNode>; links: Set<GraphLink> };
  iconCacheVersion: number;
  selectedDeviceIds: Set<string>;
  showDeviceLabels: boolean;
  showAlertBadges: boolean;
  alertBadges: Record<string, DeviceAlertBadge>;
}
export function paintGraphNode(
  node: GraphNode,
  ctx: CanvasRenderingContext2D,
  globalScale: number,
  state: GraphRenderState,
  pulseClock: number,
) {
  const {
    activeHoverNode,
    selectedNodeId,
    hoverHighlight,
    iconCacheVersion,
    selectedDeviceIds,
    showDeviceLabels,
    showAlertBadges,
    alertBadges,
  } = state;
  const isFleet = node.type === 'fleet';
  const isExternal = node.type === 'external';
  const isHovered = node === activeHoverNode;
  const isSelected = node.id === selectedNodeId;
  const isHighlighted = hoverHighlight.nodes.has(node);
  const shouldDim = activeHoverNode && !isHighlighted;

  if (node.x == null || node.y == null) return;

  const iconsReady = iconCacheVersion > 0;
  ctx.globalAlpha = shouldDim ? DIM_OPACITY : 1;

  const radius = nodeRadius(node, isHovered);

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
    ctx.font = FLEET_LABEL_FONT;
    const {
      width: rectW,
      height: rectH,
      barHeight,
    } = fleetGeometry(ctx.measureText(node.name).width, isHovered);
    const rx = node.x! - rectW / 2;
    const ry = node.y! - rectH / 2;

    ctx.fillStyle = node.color;
    ctx.fillRect(rx, ry, rectW, rectH);

    ctx.shadowBlur = 0;

    if (node.tierRatios) {
      const scaledBarH = barHeight;
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

    ctx.strokeStyle = isSelected ? SELECTION_COLOR : 'rgba(0,0,0,0.6)';
    ctx.lineWidth = isSelected ? 2 : 0.5;
    ctx.strokeRect(rx, ry, rectW, rectH);

    ctx.font = FLEET_LABEL_FONT;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillStyle = '#ffffff';
    ctx.fillText(node.name, node.x!, node.y! - barHeight / 2);

    if (node.deviceCount != null) {
      ctx.font = '8px -apple-system, sans-serif';
      ctx.fillStyle = 'rgba(255,255,255,0.6)';
      ctx.fillText(
        `${node.deviceCount} device${node.deviceCount === 1 ? '' : 's'}`,
        node.x!,
        node.y! + rectH / 2 + 10,
      );
    }
  } else if (isExternal) {
    ctx.shadowBlur = 0;
    const typeColor = node.deviceTypeColor ?? node.color;
    const side = externalNodeSide(radius);
    const rx = node.x! - side / 2;
    const ry = node.y! - side / 2;

    drawRoundedRect(ctx, rx, ry, side, side, 0);
    ctx.fillStyle = colorWithAlpha(typeColor, shouldDim ? 0.05 : 0.18);
    ctx.fill();
    ctx.strokeStyle = shouldDim
      ? colorWithAlpha(typeColor, DIM_OPACITY)
      : isSelected
        ? SELECTION_COLOR
        : colorWithAlpha(typeColor, isHovered ? 0.95 : 0.72);
    ctx.lineWidth = isSelected || isHovered ? 2 : 1.4;
    ctx.stroke();

    const stripHeight = Math.max(2, side * 0.14);
    drawRoundedRect(ctx, rx, ry + side - stripHeight, side, stripHeight, 0);
    ctx.fillStyle = shouldDim ? colorWithAlpha(node.color, DIM_OPACITY) : node.color;
    ctx.fill();

    const iconPaths = getIconPaths(node.deviceTypeIcon);
    ctx.fillStyle = shouldDim ? `rgba(255,255,255,${DIM_OPACITY})` : '#ffffff';
    if (iconsReady || iconPaths.length > 0) {
      drawIcon(ctx, iconPaths, node.x!, node.y! - stripHeight / 2, radius * 1.35);
    }

    if (showDeviceLabels) {
      const fontSize = Math.max(9, 11 / globalScale);
      ctx.font = `${fontSize}px -apple-system, sans-serif`;
      ctx.textAlign = 'center';
      ctx.textBaseline = 'middle';
      ctx.fillStyle = shouldDim ? `rgba(190,200,210,${DIM_OPACITY})` : 'rgba(190,200,210,0.8)';
      ctx.fillText(node.name, node.x!, node.y! + radius + fontSize + 2);
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
      const t = pulseClock / 1000;
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
      ctx.strokeRect(rx - selOffset, ry - selOffset, side + selOffset * 2, side + selOffset * 2);
    }

    const iconPaths = getIconPaths(node.deviceTypeIcon);
    const iconSize = effectiveRadius * 1.2;
    ctx.fillStyle = '#ffffff';
    if (iconsReady || iconPaths.length > 0) {
      drawIcon(ctx, iconPaths, node.x!, node.y!, iconSize);
    }

    if (showAlertBadges && node.device) {
      const badge = alertBadges[node.device.id];
      if (badge && badge.count > 0) {
        const badgeRadius = Math.max(5, 7 / Math.sqrt(globalScale));
        const badgeX = node.x! + effectiveRadius - 1;
        const badgeY = node.y! - effectiveRadius + 1;
        ctx.beginPath();
        ctx.arc(badgeX, badgeY, badgeRadius, 0, 2 * Math.PI);
        ctx.fillStyle = ALERT_BADGE_COLORS[badge.severity];
        ctx.fill();
        ctx.strokeStyle = 'rgba(0,0,0,0.75)';
        ctx.lineWidth = 1 / globalScale;
        ctx.stroke();

        ctx.font = `bold ${Math.max(7, 9 / Math.sqrt(globalScale))}px -apple-system, sans-serif`;
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        ctx.fillStyle = '#ffffff';
        ctx.fillText(badge.count > 9 ? '9+' : String(badge.count), badgeX, badgeY + 0.5);
      }
    }

    if (showDeviceLabels) {
      const fontSize = Math.max(10, 12 / globalScale);
      ctx.font = `${fontSize}px -apple-system, sans-serif`;
      ctx.fillStyle = shouldDim ? `rgba(255,255,255,${DIM_OPACITY})` : 'rgba(255,255,255,0.8)';
      ctx.fillText(node.name, node.x!, node.y! + effectiveRadius + fontSize + 2);
    }
  }

  ctx.globalAlpha = 1;
}
export function paintGraphLink(
  link: GraphLink,
  ctx: CanvasRenderingContext2D,
  state: Pick<GraphRenderState, 'activeHoverNode' | 'hoverHighlight'>,
  pulseClock: number,
) {
  const { activeHoverNode, hoverHighlight } = state;
  const isHighlighted = hoverHighlight.links.has(link);
  const shouldDim = activeHoverNode && !isHighlighted;

  if (typeof link.source === 'string' || typeof link.target === 'string') return;
  const source = link.source;
  const target = link.target;
  if (source.x == null || target.x == null) return;

  if (link.kind === 'declared') {
    ctx.beginPath();
    ctx.setLineDash([2, 5]);
    ctx.moveTo(source.x!, source.y!);
    ctx.lineTo(target.x!, target.y!);
    if (isHighlighted) {
      ctx.strokeStyle = 'rgba(138, 187, 255, 0.95)';
      ctx.lineWidth = 1.6;
      ctx.shadowColor = 'rgba(138, 187, 255, 0.35)';
      ctx.shadowBlur = 6;
    } else if (shouldDim) {
      ctx.strokeStyle = `rgba(138, 187, 255, ${DIM_OPACITY * 0.65})`;
      ctx.lineWidth = 0.7;
      ctx.shadowBlur = 0;
    } else {
      ctx.strokeStyle = 'rgba(138, 187, 255, 0.55)';
      ctx.lineWidth = 0.9;
      ctx.shadowBlur = 0;
    }
    ctx.stroke();
    ctx.setLineDash([]);
    ctx.shadowBlur = 0;
    return;
  }

  const deviceNode = source.type === 'device' ? source : target.type === 'device' ? target : null;
  const isActive = deviceNode?.status === 'online' || deviceNode?.status === 'warning';
  const isNever =
    deviceNode != null && !deviceNode.lastSeenTimestamp && deviceNode.status === 'offline';

  ctx.beginPath();
  if (isActive) {
    ctx.setLineDash([4, 4]);
    ctx.lineDashOffset = -(pulseClock / 1000) * 12;
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
}
