import { useState } from 'react';
import { Button, Alert, Popover, Menu, MenuItem, MenuDivider } from '@blueprintjs/core';
import { useFleets } from '../../hooks/use-fleets';
import {
  useBulkChangeFleet,
  useBulkDeleteDevices,
  useBulkRestartDevices,
  useBulkTriggerOta,
} from '../../hooks/use-devices';
import { useFirmwareUpdates } from '../../hooks/use-firmware-updates';
import { useSelectionStore } from '../../stores/selection-store';
import { showSuccessToast, showErrorToast, showWarningToast } from '../../utils/toaster';
import type { BulkTargeting, BulkDeviceFilters } from '../../types/api';

interface BulkActionBarProps {
  totalMatchingCount: number;
  visibleCount: number;
  currentFilters: BulkDeviceFilters;
}

export const BulkActionBar = ({
  totalMatchingCount,
  visibleCount,
  currentFilters,
}: BulkActionBarProps) => {
  const {
    selectedDeviceIds,
    isAllMatchingSelected,
    selectionFilters,
    selectAllMatching,
    clearSelection,
  } = useSelectionStore();

  const [deleteAlertOpen, setDeleteAlertOpen] = useState(false);
  const [restartAlertOpen, setRestartAlertOpen] = useState(false);

  const { data: fleets = [] } = useFleets();
  const { data: firmwareUpdates } = useFirmwareUpdates();

  const bulkFleetMutation = useBulkChangeFleet();
  const bulkDeleteMutation = useBulkDeleteDevices();
  const bulkRestartMutation = useBulkRestartDevices();
  const bulkOtaMutation = useBulkTriggerOta();

  const isAnyPending =
    bulkFleetMutation.isPending ||
    bulkDeleteMutation.isPending ||
    bulkRestartMutation.isPending ||
    bulkOtaMutation.isPending;

  const selectionLabel = isAllMatchingSelected
    ? `All ${totalMatchingCount} matching devices selected`
    : `${selectedDeviceIds.size} device${selectedDeviceIds.size !== 1 ? 's' : ''} selected`;

  const confirmCount = isAllMatchingSelected ? totalMatchingCount : selectedDeviceIds.size;

  function buildTargeting(): BulkTargeting {
    if (isAllMatchingSelected) {
      return { select_all: true, filters: selectionFilters ?? currentFilters };
    }
    return { device_ids: Array.from(selectedDeviceIds) };
  }

  async function handleFleetChange(fleetId: number | null) {
    try {
      const result = await bulkFleetMutation.mutateAsync({
        ...buildTargeting(),
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

  async function handleDelete() {
    try {
      const result = await bulkDeleteMutation.mutateAsync(buildTargeting());
      void showSuccessToast(`${result.affected} device${result.affected !== 1 ? 's' : ''} deleted`);
      clearSelection();
    } catch {
      void showErrorToast('Failed to delete devices');
    }
    setDeleteAlertOpen(false);
  }

  async function handleRestart() {
    try {
      const result = await bulkRestartMutation.mutateAsync(buildTargeting());
      if (result.failed > 0) {
        void showWarningToast(`${result.succeeded} restarted, ${result.failed} failed`);
        // Narrow selection to failed devices
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
        ...buildTargeting(),
        firmware_update_id: firmwareUpdateId,
      });
      if (result.failed > 0) {
        void showWarningToast(
          `${result.succeeded} updated, ${result.failed} failed (${result.errors[0]?.error ?? 'unknown error'})`,
        );
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

  const showSelectAllBanner =
    !isAllMatchingSelected &&
    selectedDeviceIds.size === visibleCount &&
    visibleCount > 0 &&
    totalMatchingCount > visibleCount;

  return (
    <div className="bulk-action-bar">
      <div className="bulk-action-bar-left">
        <span className="bulk-selection-label">{selectionLabel}</span>
        <Button icon="cross" minimal small onClick={clearSelection} title="Clear selection" />

        {showSelectAllBanner && (
          <Button minimal small intent="primary" onClick={() => selectAllMatching(currentFilters)}>
            Select all {totalMatchingCount} matching devices
          </Button>
        )}
      </div>

      <div className="bulk-action-bar-right">
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
            icon="flows"
            text="Change Fleet"
            loading={bulkFleetMutation.isPending}
            disabled={isAnyPending}
          />
        </Popover>

        <Button
          icon="refresh"
          text="Restart"
          loading={bulkRestartMutation.isPending}
          disabled={isAnyPending}
          onClick={() => setRestartAlertOpen(true)}
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
            icon="cloud-upload"
            text="Update Firmware"
            loading={bulkOtaMutation.isPending}
            disabled={isAnyPending}
          />
        </Popover>

        <Button
          icon="trash"
          text="Delete"
          intent="danger"
          loading={bulkDeleteMutation.isPending}
          disabled={isAnyPending}
          onClick={() => setDeleteAlertOpen(true)}
        />
      </div>

      {/* Delete Confirmation */}
      <Alert
        isOpen={deleteAlertOpen}
        icon="trash"
        intent="danger"
        confirmButtonText="Delete"
        cancelButtonText="Cancel"
        onConfirm={() => void handleDelete()}
        onCancel={() => setDeleteAlertOpen(false)}
      >
        <p>
          Delete <strong>{confirmCount}</strong> device{confirmCount !== 1 ? 's' : ''}? This action
          cannot be undone.
        </p>
      </Alert>

      {/* Restart Confirmation */}
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
          Restart <strong>{confirmCount}</strong> device{confirmCount !== 1 ? 's' : ''}?
        </p>
      </Alert>
    </div>
  );
};
