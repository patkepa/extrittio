import { useEffect, useRef } from 'react';
import { Button, Callout, H3, Spinner } from '@blueprintjs/core';
import { AddDeviceDialog } from '../components/devices/add-device-dialog';
import { DeviceFilters } from '../features/devices/components/device-filters';
import { DeviceTable } from '../features/devices/components/device-table';
import { useDeviceListState } from '../features/devices/hooks/use-device-list-state';
import { useFleets } from '../hooks/use-fleets';
import { useRovingFocus } from '../hooks/use-roving-focus';
import { useSelectionStore } from '../stores/selection-store';
import { useUIStore } from '../stores/ui-store';
import { getDirectionalKey, shouldIgnorePageShortcut } from '../utils/keyboard';
import './devices.css';

export const Devices = () => {
  const searchInputRef = useRef<HTMLInputElement | null>(null);
  const { openAddDeviceDialog } = useUIStore();
  const {
    selectedDeviceIds,
    isAllMatchingSelected,
    toggleDevice,
    selectAllVisible,
    deselectAllVisible,
    clearSelection,
    isSelected,
  } = useSelectionStore();
  const listState = useDeviceListState();
  const { data: fleets = [] } = useFleets();
  const {
    devices,
    devicesQuery,
    totalDeviceCount,
    searchQuery,
    setSearchQuery,
    filterStatus,
    setFilterStatus,
    filterFleetId,
    setFilterFleetId,
    sortField,
    sortDir,
    currentFilters,
    filteredDevices,
    statusCounts,
    handleSort,
    handleViewDevice,
  } = listState;

  const hasSelection = selectedDeviceIds.size > 0 || isAllMatchingSelected;
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

      if (event.key === ' ' && isRowFocused) {
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
    clearSelection,
    filteredDevices,
    focusRowIndex,
    handleViewDevice,
    hasSelection,
    toggleDevice,
  ]);

  if (devicesQuery.error) {
    return (
      <div className="devices-page">
        <Callout intent="danger" icon="error">
          Failed to load devices. Is the backend running?
        </Callout>
      </div>
    );
  }

  if (devicesQuery.isLoading) {
    return (
      <div className="devices-page">
        <Spinner />
      </div>
    );
  }

  return (
    <div className="devices-page">
      <div className="page-header">
        <div>
          <H3>Devices</H3>
          <p className="page-description">
            {filteredDevices.length} of {devices.length} devices
            {activeFleetName && (
              <span>
                {' '}
                in <strong>{activeFleetName}</strong>
              </span>
            )}
          </p>
        </div>
        <Button intent="primary" icon="add" onClick={() => openAddDeviceDialog()}>
          Add Device
        </Button>
      </div>

      <DeviceFilters
        searchInputRef={searchInputRef}
        searchQuery={searchQuery}
        onSearchQueryChange={setSearchQuery}
        filterStatus={filterStatus}
        onFilterStatusChange={setFilterStatus}
        filterFleetId={filterFleetId}
        onFilterFleetIdChange={setFilterFleetId}
        fleets={fleets}
        statusCounts={statusCounts}
        hasSelection={hasSelection}
        totalMatchingCount={totalDeviceCount}
        visibleCount={filteredDevices.length}
        currentFilters={currentFilters}
      />

      <DeviceTable
        devices={filteredDevices}
        sortField={sortField}
        sortDir={sortDir}
        activeRowIndex={activeRowIndex}
        getRowProps={getRowProps}
        registerRow={registerRow}
        isSelected={isSelected}
        onSort={handleSort}
        onViewDevice={handleViewDevice}
        onToggleDevice={toggleDevice}
        onSelectAllVisible={selectAllVisible}
        onDeselectAllVisible={deselectAllVisible}
      />

      <AddDeviceDialog />
    </div>
  );
};
