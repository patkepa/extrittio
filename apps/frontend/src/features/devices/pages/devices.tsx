import { useEffect, useRef } from 'react';
import { Button, Callout, H3, Spinner } from '@blueprintjs/core';
import { AddDeviceDialog } from '../../../components/devices/add-device-dialog';
import { DeviceFilters } from '../components/device-filters';
import { DeviceTable } from '../components/device-table';
import { useDeviceListState } from '../hooks/use-device-list-state';
import { useFleets } from '../../../hooks/use-fleets';
import {
  getDirectionalKey,
  shouldIgnorePageShortcut,
  useRovingFocus,
} from '@patkepa/kantzen-ui/interactions';
import { useSelectionStore } from '../../../stores/selection-store';
import { useUIStore } from '../../../stores/ui-store';
import { hasPermission } from '../../../auth/permissions';
import { useAuthStore } from '../../../stores/auth-store';
import './devices.css';

export const Devices = () => {
  const searchInputRef = useRef<HTMLInputElement | null>(null);
  const { openAddDeviceDialog } = useUIStore();
  const permissions = useAuthStore((state) => state.user?.permissions);
  const canManageDevices = hasPermission(permissions, 'devices.manage');
  const { selectedDeviceIds, toggleDevice, addToSelection, removeFromSelection, clearSelection } =
    useSelectionStore();
  const listState = useDeviceListState();
  const { data: fleets = [] } = useFleets();
  const {
    devices,
    devicesQuery,
    total,
    page,
    pageSize,
    setPage,
    searchQuery,
    setSearchQuery,
    filterStatus,
    setFilterStatus,
    filterFleetId,
    setFilterFleetId,
    sortField,
    sortDir,
    filteredDevices,
    handleSort,
    handleViewDevice,
  } = listState;

  const hasSelection = canManageDevices && selectedDeviceIds.size > 0;
  const activeFleetName = filterFleetId
    ? fleets.find((fleet) => fleet.id === filterFleetId)?.name
    : null;

  const {
    activeIndex: activeRowIndex,
    focusIndex: focusRowIndex,
    getItemProps: getRowProps,
    registerItem: registerRow,
  } = useRovingFocus({ itemCount: filteredDevices.length });

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (shouldIgnorePageShortcut(event)) return;

      if (event.key === '/') {
        event.preventDefault();
        searchInputRef.current?.focus();
        return;
      }

      if (event.key === 'Escape') {
        if (hasSelection) {
          event.preventDefault();
          clearSelection();
          return;
        }

        if (document.activeElement instanceof HTMLElement) {
          document.activeElement.blur();
        }
        return;
      }

      if (filteredDevices.length === 0) return;

      const activeElement = document.activeElement;
      const isRowFocused =
        activeElement instanceof HTMLElement && activeElement.hasAttribute('data-roving-item');

      if (event.key === 'Enter' && isRowFocused) {
        const device = filteredDevices[activeRowIndex];
        if (!device) return;
        event.preventDefault();
        handleViewDevice(device);
        return;
      }

      if (canManageDevices && event.key === ' ' && isRowFocused) {
        const device = filteredDevices[activeRowIndex];
        if (!device) return;
        event.preventDefault();
        toggleDevice(device.id);
        return;
      }

      const direction = getDirectionalKey(event);
      if (!direction || direction === 'left' || direction === 'right') return;

      event.preventDefault();
      if (!isRowFocused) {
        if (direction === 'last') {
          focusRowIndex(filteredDevices.length - 1);
        } else {
          focusRowIndex(direction === 'first' ? 0 : activeRowIndex);
        }
        return;
      }

      if (direction === 'up') focusRowIndex(activeRowIndex - 1);
      if (direction === 'down') focusRowIndex(activeRowIndex + 1);
      if (direction === 'first') focusRowIndex(0);
      if (direction === 'last') focusRowIndex(filteredDevices.length - 1);
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [
    activeRowIndex,
    canManageDevices,
    clearSelection,
    filteredDevices,
    focusRowIndex,
    handleViewDevice,
    hasSelection,
    toggleDevice,
  ]);

  return (
    <div className="devices-page">
      <div className="page-header">
        <div>
          <H3>Devices</H3>
          <p className="page-description">
            {devicesQuery.isPending
              ? 'Loading devices…'
              : devicesQuery.isError
                ? 'Device list unavailable'
                : `${total === 0 ? '0' : `${page * pageSize + 1}–${page * pageSize + devices.length}`} of ${total} devices`}
            {activeFleetName && (
              <span>
                {' '}
                in <strong>{activeFleetName}</strong>
              </span>
            )}
          </p>
        </div>
        {canManageDevices && (
          <Button intent="primary" icon="add" onClick={() => openAddDeviceDialog()}>
            Add Device
          </Button>
        )}
      </div>

      {devicesQuery.isError && (
        <Callout intent="danger" icon="error" className="devices-load-error">
          Could not load devices.{' '}
          <Button minimal small icon="refresh" onClick={() => void devicesQuery.refetch()}>
            Retry
          </Button>
        </Callout>
      )}
      {devicesQuery.isPending && (
        <div className="devices-loading">
          <Spinner /> Loading devices…
        </div>
      )}
      {!devicesQuery.isError && !devicesQuery.isPending && (
        <>
          <DeviceFilters
            searchInputRef={searchInputRef}
            searchQuery={searchQuery}
            onSearchQueryChange={setSearchQuery}
            filterStatus={filterStatus}
            onFilterStatusChange={setFilterStatus}
            filterFleetId={filterFleetId}
            onFilterFleetIdChange={setFilterFleetId}
            fleets={fleets}
            hasSelection={hasSelection}
          />

          <DeviceTable
            key={page}
            devices={filteredDevices}
            canSelect={canManageDevices}
            selectedDeviceIds={selectedDeviceIds}
            sortField={sortField}
            sortDir={sortDir}
            activeRowIndex={activeRowIndex}
            getRowProps={getRowProps}
            registerRow={registerRow}
            onSort={handleSort}
            onViewDevice={handleViewDevice}
            onToggleDevice={toggleDevice}
            onSelectAllVisible={addToSelection}
            onDeselectAllVisible={removeFromSelection}
          />

          {total > pageSize && (
            <div className="devices-pagination">
              <Button
                icon="chevron-left"
                disabled={page === 0 || devicesQuery.isFetching}
                onClick={() => setPage(page - 1)}
              >
                Previous
              </Button>
              <span>
                Page {page + 1} of {Math.ceil(total / pageSize)}
              </span>
              <Button
                rightIcon="chevron-right"
                disabled={(page + 1) * pageSize >= total || devicesQuery.isFetching}
                onClick={() => setPage(page + 1)}
              >
                Next
              </Button>
            </div>
          )}
        </>
      )}

      {canManageDevices && <AddDeviceDialog />}
    </div>
  );
};
