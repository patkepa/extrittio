import { useEffect, useMemo, useState } from 'react';
import { List } from 'react-window';
import type { GraphNode } from './build-force-graph-data';
import { getHealthTier, getStalenessColor, formatStaleness } from './health-utils';
import { TIER_COLORS, type HealthTier } from './constants';
import { FleetGraphMinimap, type ViewportInfo } from './fleet-graph-minimap';

interface DeviceEntry {
  node: GraphNode;
  stalenessMs: number;
  status?: string;
}

interface HealthPanelProps {
  nodes: GraphNode[];
  onDeviceClick: (nodeId: string) => void;
  selectedNodeId?: string | null;
  height: number;
  viewportRef: React.RefObject<ViewportInfo | null>;
  minimapDrawRef: React.MutableRefObject<(() => void) | null>;
  canvasWidth: number;
  canvasHeight: number;
  onMinimapNavigate: (worldX: number, worldY: number) => void;
}

interface RowExtraProps {
  deviceEntries: DeviceEntry[];
  selectedNodeId?: string | null;
  onDeviceClick: (nodeId: string) => void;
}

const HEADER_HEIGHT = 48;
const MINIMAP_SECTION_HEIGHT = 112;
const FOOTER_HEIGHT = 32;
const ROW_HEIGHT = 48;

function HealthRow({
  index,
  style,
  deviceEntries,
  selectedNodeId,
  onDeviceClick,
}: {
  index: number;
  style: React.CSSProperties;
  deviceEntries: DeviceEntry[];
  selectedNodeId?: string | null;
  onDeviceClick: (nodeId: string) => void;
}) {
  const entry = deviceEntries[index];
  if (!entry) return null;
  const { node, stalenessMs, status } = entry;
  const color = getStalenessColor(stalenessMs, status);
  const isSelected = node.id === selectedNodeId;

  return (
    <div
      style={style}
      className={`health-panel-row ${isSelected ? 'health-panel-row--selected' : ''}`}
      onClick={() => onDeviceClick(node.id)}
    >
      <span className="health-panel-led" style={{ backgroundColor: color, boxShadow: `0 0 4px ${color}80` }} />
      <span className="health-panel-name">{node.name}</span>
      <span className="health-panel-staleness" style={{ color }}>{formatStaleness(stalenessMs)}</span>
      <span className="health-panel-fleet">{node.device?.fleet_name ?? '—'}</span>
    </div>
  );
}

export const HealthPanel = ({ nodes, onDeviceClick, selectedNodeId, height, viewportRef, minimapDrawRef, canvasWidth, canvasHeight, onMinimapNavigate }: HealthPanelProps) => {
  // Tick every 5s so staleness labels and sort order stay reasonably fresh
  const [tick, setTick] = useState(0);
  useEffect(() => {
    const id = setInterval(() => setTick((t) => t + 1), 5_000);
    return () => clearInterval(id);
  }, []);

  // Filter device nodes and compute current staleness
  const deviceEntries = useMemo(() => {
    const now = Date.now();
    return nodes
      .filter((n) => n.type === 'device')
      .map((n) => {
        const stalenessMs = n.lastSeenTimestamp ? now - n.lastSeenTimestamp : NaN;
        return { node: n, stalenessMs, status: n.status };
      })
      .sort((a, b) => {
        const aVal = Number.isNaN(a.stalenessMs) ? Infinity : a.stalenessMs;
        const bVal = Number.isNaN(b.stalenessMs) ? Infinity : b.stalenessMs;
        if (aVal !== bVal) return aVal - bVal;
        return (b.node.uptimeSeconds ?? 0) - (a.node.uptimeSeconds ?? 0);
      });
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [nodes, tick]);

  // Tier counts
  const tierCounts = useMemo(() => {
    const counts: Record<HealthTier, number> = { fresh: 0, warm: 0, stale: 0, dead: 0 };
    for (const { stalenessMs, status } of deviceEntries) {
      counts[getHealthTier(stalenessMs, status)]++;
    }
    return counts;
  }, [deviceEntries]);

  const total = deviceEntries.length;

  const summaryRatios = useMemo(() => {
    if (total === 0) return { fresh: 0, warm: 0, stale: 0, dead: 0 };
    return {
      fresh: tierCounts.fresh / total,
      warm: tierCounts.warm / total,
      stale: tierCounts.stale / total,
      dead: tierCounts.dead / total,
    };
  }, [tierCounts, total]);

  const listHeight = height - HEADER_HEIGHT - FOOTER_HEIGHT - MINIMAP_SECTION_HEIGHT;

  const rowProps: RowExtraProps = useMemo(
    () => ({ deviceEntries, selectedNodeId, onDeviceClick }),
    [deviceEntries, selectedNodeId, onDeviceClick],
  );

  return (
    <div className="health-panel">
      <div className="health-panel-header">
        <span className="health-panel-title">Device Health</span>
        <div className="health-panel-summary-bar">
          {summaryRatios.fresh > 0 && (
            <div style={{ flex: summaryRatios.fresh, backgroundColor: TIER_COLORS.fresh }} />
          )}
          {summaryRatios.warm > 0 && (
            <div style={{ flex: summaryRatios.warm, backgroundColor: TIER_COLORS.warm }} />
          )}
          {summaryRatios.stale > 0 && (
            <div style={{ flex: summaryRatios.stale, backgroundColor: TIER_COLORS.stale }} />
          )}
          {summaryRatios.dead > 0 && (
            <div style={{ flex: summaryRatios.dead, backgroundColor: TIER_COLORS.dead }} />
          )}
        </div>
      </div>

      {listHeight > 0 && (
        <List<RowExtraProps>
          rowComponent={HealthRow}
          rowCount={deviceEntries.length}
          rowHeight={ROW_HEIGHT}
          rowProps={rowProps}
          style={{ height: listHeight, width: '100%' }}
        />
      )}

      <div className="health-panel-footer">
        <span style={{ color: TIER_COLORS.fresh }}>{tierCounts.fresh} fresh</span>
        <span className="health-panel-dot">&middot;</span>
        <span style={{ color: TIER_COLORS.warm }}>{tierCounts.warm + tierCounts.stale} stale</span>
        <span className="health-panel-dot">&middot;</span>
        <span style={{ color: TIER_COLORS.dead }}>{tierCounts.dead} dead</span>
      </div>

      <div className="health-panel-minimap">
        <FleetGraphMinimap
          nodes={nodes}
          viewportRef={viewportRef}
          canvasWidth={canvasWidth}
          canvasHeight={canvasHeight}
          onNavigate={onMinimapNavigate}
          drawRef={minimapDrawRef}
        />
      </div>
    </div>
  );
};
