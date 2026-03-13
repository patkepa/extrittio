import {
  useQuery,
  useMutation,
  useQueryClient,
} from "@tanstack/react-query";
import type {
  ListDevicesParams,
  CreateDeviceRequest,
  UpdateDeviceRequest,
  BulkTargeting,
  BulkFleetRequest,
  BulkOtaRequest,
} from "../types/api";
import {
  getDevices,
  getDevice,
  createDevice,
  updateDevice,
  deleteDevice,
  restartDevice,
  bulkChangeFleet,
  bulkDeleteDevices,
  bulkRestartDevices,
  bulkTriggerOta,
} from "../api/devices";
import { queryKeys } from "./query-keys";

export function useDevices(params?: ListDevicesParams) {
  return useQuery({
    queryKey: queryKeys.devices.list(params),
    queryFn: () => getDevices(params),
    staleTime: 30_000,
  });
}

export function useDevice(id: string | null) {
  return useQuery({
    queryKey: queryKeys.devices.detail(id ?? ""),
    queryFn: () => getDevice(id!),
    enabled: !!id,
    staleTime: 30_000,
  });
}

export function useCreateDevice() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: CreateDeviceRequest) => createDevice(body),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.devices.all });
      void queryClient.invalidateQueries({ queryKey: queryKeys.dashboard.stats });
      void queryClient.invalidateQueries({ queryKey: queryKeys.fleets.all });
    },
  });
}

export function useUpdateDevice() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, body }: { id: string; body: UpdateDeviceRequest }) =>
      updateDevice(id, body),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.devices.all });
      void queryClient.invalidateQueries({ queryKey: queryKeys.devices.detailAll });
    },
  });
}

export function useDeleteDevice() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => deleteDevice(id),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.devices.all });
      void queryClient.invalidateQueries({ queryKey: queryKeys.dashboard.stats });
      void queryClient.invalidateQueries({ queryKey: queryKeys.fleets.all });
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

// ---------------------------------------------------------------------------
// Bulk operations
// ---------------------------------------------------------------------------

export function useBulkChangeFleet() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: BulkFleetRequest) => bulkChangeFleet(body),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.devices.all });
      void queryClient.invalidateQueries({ queryKey: queryKeys.dashboard.stats });
      void queryClient.invalidateQueries({ queryKey: queryKeys.fleets.all });
    },
  });
}

export function useBulkDeleteDevices() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: BulkTargeting) => bulkDeleteDevices(body),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.devices.all });
      void queryClient.invalidateQueries({ queryKey: queryKeys.dashboard.stats });
      void queryClient.invalidateQueries({ queryKey: queryKeys.fleets.all });
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
