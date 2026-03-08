import {
  useQuery,
  useMutation,
  useQueryClient,
} from "@tanstack/react-query";
import type { ListDevicesParams, CreateDeviceRequest, UpdateDeviceRequest } from "../types/api";
import {
  getDevices,
  getDevice,
  createDevice,
  updateDevice,
  deleteDevice,
  restartDevice,
} from "../api/devices";

export function useDevices(params?: ListDevicesParams) {
  return useQuery({
    queryKey: ["devices", params],
    queryFn: () => getDevices(params),
    staleTime: 30_000,
  });
}

export function useDevice(id: string | null) {
  return useQuery({
    queryKey: ["device", id],
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
      void queryClient.invalidateQueries({ queryKey: ["devices"] });
      void queryClient.invalidateQueries({ queryKey: ["dashboard-stats"] });
    },
  });
}

export function useUpdateDevice() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, body }: { id: string; body: UpdateDeviceRequest }) =>
      updateDevice(id, body),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["devices"] });
      void queryClient.invalidateQueries({ queryKey: ["device"] });
    },
  });
}

export function useDeleteDevice() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => deleteDevice(id),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["devices"] });
      void queryClient.invalidateQueries({ queryKey: ["dashboard-stats"] });
    },
  });
}

export function useRestartDevice() {
  return useMutation({
    mutationFn: (id: string) => restartDevice(id),
  });
}
