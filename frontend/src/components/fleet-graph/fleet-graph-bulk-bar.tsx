import { useRef, useState } from 'react';
import { Button, Alert, Popover, Menu, MenuItem, MenuDivider } from '@blueprintjs/core';
import { useFleets } from '../../hooks/use-fleets';
import {
  useBulkChangeFleet,
  useBulkRestartDevices,
  useBulkTriggerOta,
} from '../../hooks/use-devices';
import { useFirmwareUpdates } from '../../hooks/use-firmware-updates';
import { useSelectionStore } from '../../stores/selection-store';
import { showSuccessToast, showErrorToast, showWarningToast } from '../../utils/toaster';

export const FleetGraphBulkBar = () => {
  const { selectedDeviceIds, clearSelection } = useSelectionStore();
  const count = selectedDeviceIds.size;

  const [restartAlertOpen, setRestartAlertOpen] = useState(false);
  // Snapshot device IDs when the restart dialog opens so the confirmation
  // always acts on the selection that was confirmed, not a later mutation.
  const restartIdsRef = useRef<string[]>([]);

  const { data: fleets = [] } = useFleets();
  const { data: firmwareUpdates } = useFirmwareUpdates();

  const bulkFleetMutation = useBulkChangeFleet();
  const bulkRestartMutation = useBulkRestartDevices();
  const bulkOtaMutation = useBulkTriggerOta();

  const isAnyPending =
    bulkFleetMutation.isPending || bulkRestartMutation.isPending || bulkOtaMutation.isPending;

  if (count === 0) return null;

  const deviceIds = Array.from(selectedDeviceIds).map((id) => id.replace(/^device-/, ''));

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
      clearSelection();
    } catch {
      void showErrorToast('Failed to change fleet');
    }
  }

  async function handleRestart() {
    try {
      const result = await bulkRestartMutation.mutateAsync({ device_ids: restartIdsRef.current });
      if (result.failed > 0) {
        void showWarningToast(`${result.succeeded} restarted, ${result.failed} failed`);
        useSelectionStore.setState({
          selectedDeviceIds: new Set(result.errors.map((e) => e.device_id)),
          isAllMatchingSelected: false,
          selectionFilters: null,
        });
      } else {
        void showSuccessToast(
          `${result.succeeded} device${result.succeeded !== 1 ? 's' : ''} restarted`,
        );
        clearSelection();
      }
    } catch {
      void showErrorToast('Failed to restart devices');
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
        useSelectionStore.setState({
          selectedDeviceIds: new Set(result.errors.map((e) => e.device_id)),
          isAllMatchingSelected: false,
          selectionFilters: null,
        });
      } else {
        void showSuccessToast(
          `OTA triggered on ${result.succeeded} device${result.succeeded !== 1 ? 's' : ''}`,
        );
        clearSelection();
      }
    } catch {
      void showErrorToast('Failed to trigger OTA update');
    }
  }

  return (
    <div className="fleet-graph-bulk-bar" role="toolbar" aria-label="Bulk device actions">
      <div className="fleet-graph-bulk-bar-left">
        <span className="fleet-graph-bulk-label">
          {count} device{count !== 1 ? 's' : ''} selected
        </span>
        <Button icon="cross" minimal small onClick={clearSelection} title="Clear selection" />
      </div>

      <div className="fleet-graph-bulk-bar-right">
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
          placement="top"
          disabled={isAnyPending}
        >
          <Button
            icon="flows"
            text="Change Fleet"
            small
            loading={bulkFleetMutation.isPending}
            disabled={isAnyPending}
          />
        </Popover>

        <Button
          icon="refresh"
          text="Restart"
          small
          loading={bulkRestartMutation.isPending}
          disabled={isAnyPending}
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
          placement="top"
          disabled={isAnyPending}
        >
          <Button
            icon="cloud-upload"
            text="Update Firmware"
            small
            loading={bulkOtaMutation.isPending}
            disabled={isAnyPending}
          />
        </Popover>
      </div>

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
          Restart <strong>{count}</strong> device{count !== 1 ? 's' : ''}?
        </p>
      </Alert>
    </div>
  );
};
