import { useCallback, useEffect, useMemo, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { useSelectionStore } from '../../../stores/selection-store';
import type { BulkDeviceFilters, Device, ListDevicesParams } from '../../../types/api';
import { useAllDevices } from '../queries/use-devices';
import {
  countDeviceStatuses,
  filterAndSortDevices,
  type DeviceSortDir,
  type DeviceSortField,
  type DeviceStatusFilter,
} from '../model/device-list';

export type { DeviceSortDir, DeviceSortField, DeviceStatusFilter } from '../model/device-list';

export function useDeviceListState() {
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();
  const clearSelection = useSelectionStore((state) => state.clearSelection);
  const [searchQuery, setSearchQuery] = useState('');
  const [filterStatus, setFilterStatus] = useState<DeviceStatusFilter>('all');
  const [sortField, setSortField] = useState<DeviceSortField>('last_seen');
  const [sortDir, setSortDir] = useState<DeviceSortDir>('desc');
  const [debouncedSearch, setDebouncedSearch] = useState('');

  const filterFleetId = searchParams.get('fleet_id') ? Number(searchParams.get('fleet_id')) : null;

  const setFilterFleetId = useCallback(
    (fleetId: number | null) => {
      setSearchParams((prev) => {
        const next = new URLSearchParams(prev);
        if (fleetId === null) {
          next.delete('fleet_id');
        } else {
          next.set('fleet_id', String(fleetId));
        }
        return next;
      });
    },
    [setSearchParams],
  );

  useEffect(() => {
    clearSelection();
  }, [filterStatus, searchQuery, filterFleetId, clearSelection]);

  useEffect(() => {
    const timer = setTimeout(() => setDebouncedSearch(searchQuery), 300);
    return () => clearTimeout(timer);
  }, [searchQuery]);

  const queryParams = useMemo<ListDevicesParams | undefined>(() => {
    const params: ListDevicesParams = {
      ...(filterFleetId ? { fleet_id: filterFleetId } : {}),
      ...(debouncedSearch ? { search: debouncedSearch } : {}),
    };
    return Object.keys(params).length > 0 ? params : undefined;
  }, [debouncedSearch, filterFleetId]);

  const devicesQuery = useAllDevices(queryParams);
  const devices = useMemo(() => devicesQuery.data?.data ?? [], [devicesQuery.data?.data]);
  const totalDeviceCount = devicesQuery.data?.total ?? 0;

  const currentFilters = useMemo<BulkDeviceFilters>(
    () => ({
      ...(filterStatus !== 'all' ? { status: filterStatus } : {}),
      ...(searchQuery ? { search: searchQuery } : {}),
      ...(filterFleetId ? { fleet_id: filterFleetId } : {}),
    }),
    [filterFleetId, filterStatus, searchQuery],
  );

  const deviceParam = searchParams.get('device');
  useEffect(() => {
    if (!deviceParam || devices.length === 0) return;

    const device = devices.find((d) => d.id === deviceParam);
    if (device) {
      navigate(`/devices/${device.id}`, { replace: true });
      return;
    }

    setSearchParams(
      (prev) => {
        const next = new URLSearchParams(prev);
        next.delete('device');
        return next;
      },
      { replace: true },
    );
  }, [deviceParam, devices, navigate, setSearchParams]);

  const filteredDevices = useMemo(
    () => filterAndSortDevices(devices, filterStatus, sortField, sortDir),
    [devices, filterStatus, sortDir, sortField],
  );

  const statusCounts = useMemo(() => countDeviceStatuses(devices), [devices]);

  const handleSort = useCallback(
    (field: DeviceSortField) => {
      if (sortField === field) {
        setSortDir(sortDir === 'asc' ? 'desc' : 'asc');
      } else {
        setSortField(field);
        setSortDir('asc');
      }
    },
    [sortDir, sortField],
  );

  const handleViewDevice = useCallback(
    (device: Device) => {
      navigate(`/devices/${device.id}`);
    },
    [navigate],
  );

  return {
    devicesQuery,
    devices,
    totalDeviceCount,
    searchQuery,
    setSearchQuery,
    filterStatus,
    setFilterStatus,
    filterFleetId,
    setFilterFleetId,
    sortField,
    sortDir,
    queryParams,
    currentFilters,
    filteredDevices,
    statusCounts,
    handleSort,
    handleViewDevice,
  };
}
