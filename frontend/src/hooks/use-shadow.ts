import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import {
  getDeviceShadow,
  updateDesiredState,
  updateReportedState,
  deleteDeviceShadow,
} from "../api/shadows";

export function useDeviceShadow(deviceId: string | null) {
  return useQuery({
    queryKey: ["device-shadow", deviceId],
    queryFn: () => getDeviceShadow(deviceId!),
    enabled: !!deviceId,
    staleTime: 10_000,
  });
}

export function useUpdateDesiredState() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ deviceId, state }: { deviceId: string; state: Record<string, unknown> }) =>
      updateDesiredState(deviceId, state),
    onSuccess: (_data, variables) => {
      void queryClient.invalidateQueries({ queryKey: ["device-shadow", variables.deviceId] });
    },
  });
}

export function useUpdateReportedState() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ deviceId, state }: { deviceId: string; state: Record<string, unknown> }) =>
      updateReportedState(deviceId, state),
    onSuccess: (_data, variables) => {
      void queryClient.invalidateQueries({ queryKey: ["device-shadow", variables.deviceId] });
    },
  });
}

export function useDeleteDeviceShadow() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (deviceId: string) => deleteDeviceShadow(deviceId),
    onSuccess: (_data, deviceId) => {
      void queryClient.invalidateQueries({ queryKey: ["device-shadow", deviceId] });
    },
  });
}
