import { useEffect, useMemo, useState } from 'react';
import { List } from 'react-window';
import type { GraphNode, GraphLink } from './build-force-graph-data';
import {
  getHealthTier,
  getStalenessColor,
  formatStaleness,
  type HealthStatusFilter,
} from './health-utils';
import { TIER_COLORS, type HealthTier } from './constants';
import { FleetGraphMinimap, type ViewportInfo } from './fleet-graph-minimap';
import { RightSidebar } from '../layout/right-sidebar';

export type HealthStatusVisibility = Record<HealthStatusFilter, boolean>;
export type HealthStatusCounts = Record<HealthStatusFilter, number>;

interface DeviceEntry {
  node: GraphNode;
  stalenessMs: number;
  status?: string;
}

interface HealthPanelProps {
  nodes: GraphNode[];
  links: GraphLink[];
  onDeviceClick: (nodeId: string) => void;
  onDeviceHover?: (nodeId: string | null) => void;
  selectedNodeId?: string | null;
  hoveredNodeId?: string | null;
  height: number;
  viewportRef: React.RefObject<ViewportInfo | null>;
  minimapDrawRef: React.MutableRefObject<(() => void) | null>;
  canvasWidth: number;
  canvasHeight: number;
  collapsed?: boolean;
  statusVisibility: HealthStatusVisibility;
  statusCounts: HealthStatusCounts;
  onStatusToggle: (status: HealthStatusFilter) => void;
}

interface RowExtraProps {
  deviceEntries: DeviceEntry[];
  selectedNodeId?: string | null;
  hoveredNodeId?: string | null;
  onDeviceClick: (nodeId: string) => void;
  onDeviceHover?: (nodeId: string | null) => void;
}

const HEADER_HEIGHT = 48;
const MINIMAP_SECTION_HEIGHT = 112;
const FOOTER_HEIGHT = 40;
const ROW_HEIGHT = 48;

const statusFilterConfig: Array<{
  key: HealthStatusFilter;
  label: string;
  color: string;
}> = [
  { key: 'connected', label: 'Connected', color: TIER_COLORS.fresh },
  { key: 'offline', label: 'Offline', color: TIER_COLORS.dead },
  { key: 'never', label: 'Never', color: TIER_COLORS.never },
];

function HealthRow({
  index,
  style,
  deviceEntries,
  selectedNodeId,
  hoveredNodeId,
  onDeviceClick,
  onDeviceHover,
}: {
  index: number;
  style: React.CSSProperties;
  deviceEntries: DeviceEntry[];
  selectedNodeId?: string | null;
  hoveredNodeId?: string | null;
  onDeviceClick: (nodeId: string) => void;
  onDeviceHover?: (nodeId: string | null) => void;
}) {
  const entry = deviceEntries[index];
  if (!entry) return null;
  const { node, stalenessMs, status } = entry;
  const color = getStalenessColor(stalenessMs, status);
  const isSelected = node.id === selectedNodeId;
  const isHovered = node.id === hoveredNodeId;

  return (
    <div
      style={style}
      className={`health-panel-row ${isSelected ? 'health-panel-row--selected' : ''} ${
        isHovered ? 'health-panel-row--hovered' : ''
      }`}
      role="button"
      tabIndex={0}
      data-right-sidebar-item="true"
      data-focus-region-initial={isSelected ? 'true' : undefined}
      onClick={() => onDeviceClick(node.id)}
      onMouseEnter={() => onDeviceHover?.(node.id)}
      onMouseLeave={() => onDeviceHover?.(null)}
      onFocus={() => onDeviceHover?.(node.id)}
      onBlur={() => onDeviceHover?.(null)}
    >
      <span
        className="health-panel-led"
        style={{ backgroundColor: color, boxShadow: `0 0 4px ${color}80` }}
      />
      <span className="health-panel-name">{node.name}</span>
      <span className="health-panel-staleness" style={{ color }}>
        {formatStaleness(stalenessMs)}
      </span>
      <span className="health-panel-fleet">{node.device?.fleet_name ?? '—'}</span>
    </div>
  );
}

export const HealthPanel = ({
  nodes,
  links,
  onDeviceClick,
  onDeviceHover,
  selectedNodeId,
  hoveredNodeId,
  height,
  viewportRef,
  minimapDrawRef,
  canvasWidth,
  canvasHeight,
  collapsed,
  statusVisibility,
  statusCounts,
  onStatusToggle,
}: HealthPanelProps) => {
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
    const counts: Record<HealthTier, number> = { fresh: 0, warm: 0, stale: 0, dead: 0, never: 0 };
    for (const { stalenessMs, status } of deviceEntries) {
      counts[getHealthTier(stalenessMs, status)]++;
    }
    return counts;
  }, [deviceEntries]);

  const total = deviceEntries.length;

  const summaryRatios = useMemo(() => {
    if (total === 0) return { fresh: 0, warm: 0, stale: 0, dead: 0, never: 0 };
    return {
      fresh: tierCounts.fresh / total,
      warm: tierCounts.warm / total,
      stale: tierCounts.stale / total,
      dead: tierCounts.dead / total,
      never: tierCounts.never / total,
    };
  }, [tierCounts, total]);

  const listHeight = height - HEADER_HEIGHT - FOOTER_HEIGHT - MINIMAP_SECTION_HEIGHT;

  const rowProps: RowExtraProps = useMemo(
    () => ({ deviceEntries, selectedNodeId, hoveredNodeId, onDeviceClick, onDeviceHover }),
    [deviceEntries, selectedNodeId, hoveredNodeId, onDeviceClick, onDeviceHover],
  );

  return (
    <RightSidebar className="health-panel" collapsed={collapsed} ariaLabel="Device health">
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
          {summaryRatios.never > 0 && (
            <div style={{ flex: summaryRatios.never, backgroundColor: TIER_COLORS.never }} />
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

      <div className="health-panel-footer right-sidebar-footer">
        {statusFilterConfig.map((filter) => {
          const isActive = statusVisibility[filter.key];
          const count = statusCounts[filter.key];
          return (
            <button
              key={filter.key}
              type="button"
              className={`health-panel-filter${isActive ? '' : ' health-panel-filter--off'}`}
              onClick={() => onStatusToggle(filter.key)}
              aria-pressed={isActive}
              disabled={count === 0}
              style={{ '--health-panel-filter-color': filter.color } as React.CSSProperties}
            >
              <span className="health-panel-filter-count">{count.toLocaleString()}</span>
              <span className="health-panel-filter-label">{filter.label}</span>
            </button>
          );
        })}
      </div>

      <div className="health-panel-minimap">
        <FleetGraphMinimap
          nodes={nodes}
          links={links}
          viewportRef={viewportRef}
          canvasWidth={canvasWidth}
          canvasHeight={canvasHeight}
          drawRef={minimapDrawRef}
        />
      </div>
    </RightSidebar>
  );
};
