import { Button, Icon } from '@blueprintjs/core';
import type { ReactNode } from 'react';
import { useNavigate } from 'react-router-dom';
import { MainToolbar } from '../layout/main-toolbar';
import type { Device } from '../../types/api';

interface FleetGraphToolbarProps {
  selectedDevice: Device | null;
  deviceCount: number;
  fleetCount: number;
  healthPanelOpen: boolean;
  hasGraphData: boolean;
  onClearDevice: () => void;
  onFitView: () => void;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onToggleHealthPanel: () => void;
}

interface ToolbarMetricProps {
  label: string;
  value: ReactNode;
}

const ToolbarMetric = ({ label, value }: ToolbarMetricProps) => (
  <div className="fleet-graph-toolbar-metric">
    <span className="fleet-graph-toolbar-metric-label">{label}</span>
    <span className="fleet-graph-toolbar-metric-value">{value}</span>
  </div>
);

export const FleetGraphToolbar = ({
  selectedDevice,
  deviceCount,
  fleetCount,
  healthPanelOpen,
  hasGraphData,
  onClearDevice,
  onFitView,
  onZoomIn,
  onZoomOut,
  onToggleHealthPanel,
}: FleetGraphToolbarProps) => {
  const navigate = useNavigate();

  return (
    <MainToolbar className="fleet-graph-toolbar-shell" ariaLabel="Fleet graph toolbar">
      <div className="fleet-graph-toolbar">
        {selectedDevice ? (
          <>
            <div className="fleet-graph-toolbar-identity">
              <span className={`status-led status-led--${selectedDevice.status}`} />
              <div className="fleet-graph-toolbar-title-group">
                <span className="fleet-graph-toolbar-title">{selectedDevice.name}</span>
                <span className="fleet-graph-toolbar-subtitle mono-data">{selectedDevice.id}</span>
              </div>
            </div>

            <div className="fleet-graph-toolbar-metrics" aria-label="Selected device summary">
              <ToolbarMetric label="Type" value={selectedDevice.device_type_name} />
              <ToolbarMetric label="Fleet" value={selectedDevice.fleet_name ?? 'Unassigned'} />
              <ToolbarMetric
                label="Firmware"
                value={<span className="mono-data">{selectedDevice.firmware}</span>}
              />
              <ToolbarMetric
                label="Last Seen"
                value={<span className="mono-data">{selectedDevice.last_seen}</span>}
              />
              <ToolbarMetric
                label="Uptime"
                value={<span className="mono-data">{selectedDevice.uptime}</span>}
              />
            </div>

            <div className="fleet-graph-toolbar-actions">
              <Button
                icon="eye-open"
                small
                intent="primary"
                onClick={() => navigate(`/devices/${selectedDevice.id}`)}
              >
                View Details
              </Button>
              <Button icon="cross" minimal small title="Clear device" onClick={onClearDevice} />
            </div>
          </>
        ) : (
          <>
            <div className="fleet-graph-toolbar-identity">
              <Icon icon="graph" size={16} />
              <div className="fleet-graph-toolbar-title-group">
                <span className="fleet-graph-toolbar-title">Fleet topology</span>
                <span className="fleet-graph-toolbar-subtitle">
                  {deviceCount.toLocaleString()} devices / {fleetCount.toLocaleString()} fleets
                </span>
              </div>
            </div>

            <div className="fleet-graph-toolbar-spacer" />

            <div className="fleet-graph-toolbar-actions">
              <Button
                icon="zoom-to-fit"
                minimal
                small
                disabled={!hasGraphData}
                title="Fit graph"
                onClick={onFitView}
              />
              <Button
                icon="plus"
                minimal
                small
                disabled={!hasGraphData}
                title="Zoom in"
                onClick={onZoomIn}
              />
              <Button
                icon="minus"
                minimal
                small
                disabled={!hasGraphData}
                title="Zoom out"
                onClick={onZoomOut}
              />
              <Button
                icon={healthPanelOpen ? 'chevron-right' : 'chevron-left'}
                minimal
                small
                disabled={!hasGraphData}
                title={healthPanelOpen ? 'Hide health panel (⇧⌘B)' : 'Show health panel (⇧⌘B)'}
                onClick={onToggleHealthPanel}
              />
            </div>
          </>
        )}
      </div>
    </MainToolbar>
  );
};
