import { useCallback, useEffect, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { useSelectionStore } from '../../../stores/selection-store';
import type { Device, ListDevicesParams } from '../../../types/api';
import { useDevices } from '../queries/use-devices';
import {
  type DeviceSortDir,
  type DeviceSortField,
  type DeviceStatusFilter,
} from '../model/device-list';

export type { DeviceSortDir, DeviceSortField, DeviceStatusFilter } from '../model/device-list';

export function useDeviceListState() {
  const pageSize = 100;
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();
  const clearSelection = useSelectionStore((state) => state.clearSelection);
  const [searchQuery, setSearchQuery] = useState('');
  const [filterStatus, setFilterStatus] = useState<DeviceStatusFilter>('all');
  const [sortField, setSortField] = useState<DeviceSortField>('last_seen');
  const [sortDir, setSortDir] = useState<DeviceSortDir>('desc');
  const [debouncedSearch, setDebouncedSearch] = useState('');
  const [page, setPage] = useState(0);

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

  const queryParams: ListDevicesParams = {
    limit: pageSize,
    offset: page * pageSize,
    sort_by: sortField,
    sort_dir: sortDir,
    ...(filterStatus === 'all' ? {} : { status: filterStatus }),
    ...(filterFleetId ? { fleet_id: filterFleetId } : {}),
    ...(debouncedSearch ? { search: debouncedSearch } : {}),
  };

  const devicesQuery = useDevices(queryParams);
  const devices = devicesQuery.data?.data ?? [];
  const total = devicesQuery.data?.total ?? 0;

  const deviceParam = searchParams.get('device');
  useEffect(() => {
    if (deviceParam) navigate(`/devices/${deviceParam}`, { replace: true });
  }, [deviceParam, navigate]);

  const filteredDevices = devices;

  const handleSort = useCallback(
    (field: DeviceSortField) => {
      if (sortField === field) {
        setSortDir(sortDir === 'asc' ? 'desc' : 'asc');
      } else {
        setSortField(field);
        setSortDir('asc');
      }
      setPage(0);
    },
    [sortDir, sortField],
  );

  const handleViewDevice = useCallback(
    (device: Device) => {
      navigate(`/devices/${device.id}`);
    },
    [navigate],
  );

  const changeSearchQuery = (value: string) => {
    setSearchQuery(value);
    setPage(0);
  };
  const changeFilterStatus = (value: DeviceStatusFilter) => {
    setFilterStatus(value);
    setPage(0);
  };
  const changeFilterFleetId = (value: number | null) => {
    setFilterFleetId(value);
    setPage(0);
  };

  return {
    devicesQuery,
    devices,
    total,
    page,
    pageSize,
    setPage,
    searchQuery,
    setSearchQuery: changeSearchQuery,
    filterStatus,
    setFilterStatus: changeFilterStatus,
    filterFleetId,
    setFilterFleetId: changeFilterFleetId,
    sortField,
    sortDir,
    queryParams,
    filteredDevices,
    handleSort,
    handleViewDevice,
  };
}
