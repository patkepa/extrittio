import { Alert, Button, Icon, Menu, MenuDivider, MenuItem, Popover } from '@blueprintjs/core';
import { useRef, useState, type ReactNode } from 'react';
import { useNavigate } from 'react-router-dom';
import { MainToolbar } from '../layout/main-toolbar';
import { useFleets } from '../../hooks/use-fleets';
import {
  useBulkChangeFleet,
  useBulkRestartDevices,
  useBulkTriggerOta,
} from '../../hooks/use-devices';
import { useFirmwareUpdates } from '../../hooks/use-firmware-updates';
import { showErrorToast, showSuccessToast, showWarningToast } from '../../utils/toaster';
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

const toolbarIconButtonClass = 'panel-toolbar-button panel-toolbar-button--icon';

interface SelectedDeviceActionsProps {
  device: Device;
}

const SelectedDeviceActions = ({ device }: SelectedDeviceActionsProps) => {
  const navigate = useNavigate();
  const restartIdsRef = useRef<string[]>([]);
  const [restartAlertOpen, setRestartAlertOpen] = useState(false);

  const { data: fleets = [] } = useFleets();
  const { data: firmwareUpdates } = useFirmwareUpdates();
  const bulkFleetMutation = useBulkChangeFleet();
  const bulkRestartMutation = useBulkRestartDevices();
  const bulkOtaMutation = useBulkTriggerOta();

  const deviceIds = [device.id];
  const isAnyPending =
    bulkFleetMutation.isPending || bulkRestartMutation.isPending || bulkOtaMutation.isPending;

  async function handleFleetChange(fleetId: number | null) {
    try {
      const result = await bulkFleetMutation.mutateAsync({
        device_ids: deviceIds,
        fleet_id: fleetId,
      });
      void showSuccessToast(
        fleetId === null
          ? `${result.affected} device${result.affected !== 1 ? 's' : ''} removed from fleet`
          : `${result.affected} device${result.affected !== 1 ? 's' : ''} moved to fleet`,
      );
    } catch {
      void showErrorToast('Failed to change fleet');
    }
  }

  async function handleRestart() {
    try {
      const result = await bulkRestartMutation.mutateAsync({ device_ids: restartIdsRef.current });
      if (result.failed > 0) {
        void showWarningToast(`${result.succeeded} restarted, ${result.failed} failed`);
      } else {
        void showSuccessToast(
          `${result.succeeded} device${result.succeeded !== 1 ? 's' : ''} restarted`,
        );
      }
    } catch {
      void showErrorToast('Failed to restart device');
    }
    setRestartAlertOpen(false);
  }

  async function handleOta(firmwareUpdateId: number) {
    try {
      const result = await bulkOtaMutation.mutateAsync({
        device_ids: deviceIds,
        firmware_update_id: firmwareUpdateId,
      });
      if (result.failed > 0) {
        void showWarningToast(`${result.succeeded} updated, ${result.failed} failed`);
      } else {
        void showSuccessToast(
          `OTA triggered on ${result.succeeded} device${result.succeeded !== 1 ? 's' : ''}`,
        );
      }
    } catch {
      void showErrorToast('Failed to trigger OTA update');
    }
  }

  return (
    <>
      <Button
        className={toolbarIconButtonClass}
        icon="eye-open"
        minimal
        small
        title="View details"
        aria-label="View details"
        onClick={() => navigate(`/devices/${device.id}`)}
      />

      <Popover
        content={
          <Menu>
            {fleets.map((fleet) => (
              <MenuItem
                key={fleet.id}
                text={fleet.name}
                onClick={() => void handleFleetChange(fleet.id)}
              />
            ))}
            {fleets.length > 0 && <MenuDivider />}
            <MenuItem
              text="Remove from fleet"
              icon="cross"
              intent="warning"
              onClick={() => void handleFleetChange(null)}
            />
          </Menu>
        }
        placement="bottom"
        disabled={isAnyPending}
      >
        <Button
          className={toolbarIconButtonClass}
          icon="flows"
          minimal
          small
          loading={bulkFleetMutation.isPending}
          disabled={isAnyPending}
          title="Change fleet"
          aria-label="Change fleet"
        />
      </Popover>

      <Button
        className={toolbarIconButtonClass}
        icon="refresh"
        minimal
        small
        loading={bulkRestartMutation.isPending}
        disabled={isAnyPending}
        title="Restart device"
        aria-label="Restart device"
        onClick={() => {
          restartIdsRef.current = deviceIds;
          setRestartAlertOpen(true);
        }}
      />

      <Popover
        content={
          <Menu>
            {(firmwareUpdates ?? []).map((fw) => (
              <MenuItem
                key={fw.id}
                text={`${fw.version} (${fw.device_type_name})`}
                onClick={() => void handleOta(fw.id)}
              />
            ))}
            {(firmwareUpdates ?? []).length === 0 && (
              <MenuItem text="No firmware updates available" disabled />
            )}
          </Menu>
        }
        placement="bottom"
        disabled={isAnyPending}
      >
        <Button
          className={toolbarIconButtonClass}
          icon="cloud-upload"
          minimal
          small
          loading={bulkOtaMutation.isPending}
          disabled={isAnyPending}
          title="Update firmware"
          aria-label="Update firmware"
        />
      </Popover>

      <Alert
        isOpen={restartAlertOpen}
        icon="refresh"
        intent="warning"
        confirmButtonText="Restart"
        cancelButtonText="Cancel"
        onConfirm={() => void handleRestart()}
        onCancel={() => setRestartAlertOpen(false)}
      >
        <p>
          Restart <strong>{device.name}</strong>?
        </p>
      </Alert>
    </>
  );
};

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

            <div className="fleet-graph-toolbar-divider" aria-hidden="true" />

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

            <div className="fleet-graph-toolbar-divider" aria-hidden="true" />

            <div className="fleet-graph-toolbar-actions">
              <SelectedDeviceActions device={selectedDevice} />
              <div className="fleet-graph-toolbar-divider" aria-hidden="true" />
              <Button
                className={toolbarIconButtonClass}
                icon="cross"
                minimal
                small
                title="Clear device"
                onClick={onClearDevice}
              />
              <div className="fleet-graph-toolbar-divider" aria-hidden="true" />
              <Button
                className={toolbarIconButtonClass}
                icon={healthPanelOpen ? 'chevron-right' : 'chevron-left'}
                minimal
                small
                disabled={!hasGraphData}
                title={healthPanelOpen ? 'Hide health panel' : 'Show health panel'}
                onClick={onToggleHealthPanel}
              />
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
                className={toolbarIconButtonClass}
                icon="zoom-to-fit"
                minimal
                small
                disabled={!hasGraphData}
                title="Fit graph"
                onClick={onFitView}
              />
              <Button
                className={toolbarIconButtonClass}
                icon="plus"
                minimal
                small
                disabled={!hasGraphData}
                title="Zoom in"
                onClick={onZoomIn}
              />
              <Button
                className={toolbarIconButtonClass}
                icon="minus"
                minimal
                small
                disabled={!hasGraphData}
                title="Zoom out"
                onClick={onZoomOut}
              />
              <div className="fleet-graph-toolbar-divider" aria-hidden="true" />
              <Button
                className={toolbarIconButtonClass}
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
