import { useQuery, useMutation, useQueryClient, type QueryClient } from '@tanstack/react-query';
import type {
  ListDevicesParams,
  CreateDeviceRequest,
  UpdateDeviceRequest,
  BulkTargeting,
  BulkFleetRequest,
  BulkOtaRequest,
} from '../../../types/api';
import {
  getDevices,
  getAllDevices,
  getDevice,
  getDeviceContract,
  createDevice,
  updateDevice,
  deleteDevice,
  restartDevice,
  bulkChangeFleet,
  bulkDeleteDevices,
  bulkRestartDevices,
  bulkTriggerOta,
} from '../api/devices-api';
import { queryKeys } from '../../../hooks/query-keys';

export function invalidateDeviceInventory(queryClient: QueryClient) {
  void queryClient.invalidateQueries({ queryKey: queryKeys.devices.all });
  void queryClient.invalidateQueries({ queryKey: queryKeys.dashboard.stats });
  void queryClient.invalidateQueries({ queryKey: queryKeys.fleets.all });
}

export function invalidateDeviceDetails(queryClient: QueryClient) {
  void queryClient.invalidateQueries({ queryKey: queryKeys.devices.detailAll });
}

export function useDevices(
  params?: ListDevicesParams,
  options?: { enabled?: boolean; refetchInterval?: number },
) {
  return useQuery({
    queryKey: queryKeys.devices.list(params),
    queryFn: () => getDevices(params),
    staleTime: 30_000,
    enabled: options?.enabled ?? true,
    refetchInterval: options?.refetchInterval,
  });
}

export function useAllDevices(
  params?: Omit<ListDevicesParams, 'limit' | 'offset'>,
  options?: { enabled?: boolean; refetchInterval?: number },
) {
  return useQuery({
    queryKey: queryKeys.devices.fullList(params),
    queryFn: () => getAllDevices(params),
    staleTime: 30_000,
    enabled: options?.enabled ?? true,
    refetchInterval: options?.refetchInterval,
  });
}

export function useDevice(id: string | null) {
  return useQuery({
    queryKey: queryKeys.devices.detail(id ?? ''),
    queryFn: () => getDevice(id!),
    enabled: !!id,
    staleTime: 30_000,
  });
}

export function useDeviceContract(id: string, options?: { retry?: boolean }) {
  return useQuery({
    queryKey: queryKeys.devices.contract(id),
    queryFn: () => getDeviceContract(id),
    enabled: id.length > 0,
    staleTime: 60_000,
    retry: options?.retry,
  });
}

export function useCreateDevice() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: CreateDeviceRequest) => createDevice(body),
    onSuccess: () => {
      invalidateDeviceInventory(queryClient);
    },
  });
}

export function useUpdateDevice() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, body }: { id: string; body: UpdateDeviceRequest }) => updateDevice(id, body),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.devices.all });
      invalidateDeviceDetails(queryClient);
    },
  });
}

export function useDeleteDevice() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => deleteDevice(id),
    onSuccess: () => {
      invalidateDeviceInventory(queryClient);
    },
  });
}

export function useRestartDevice() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => restartDevice(id),
    onSuccess: (_data, id) => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.commands.list(id) });
    },
  });
}

export function useBulkChangeFleet() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: BulkFleetRequest) => bulkChangeFleet(body),
    onSuccess: () => {
      invalidateDeviceInventory(queryClient);
    },
  });
}

export function useBulkDeleteDevices() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: BulkTargeting) => bulkDeleteDevices(body),
    onSuccess: () => {
      invalidateDeviceInventory(queryClient);
    },
  });
}

export function useBulkRestartDevices() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: BulkTargeting) => bulkRestartDevices(body),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.devices.all });
    },
  });
}

export function useBulkTriggerOta() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: BulkOtaRequest) => bulkTriggerOta(body),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.devices.all });
      void queryClient.invalidateQueries({ queryKey: queryKeys.firmware.all });
    },
  });
}
