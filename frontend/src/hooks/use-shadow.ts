import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import {
  getDeviceShadow,
  updateDesiredState,
  updateReportedState,
  deleteDeviceShadow,
} from "../api/shadows";
import { queryKeys } from "./query-keys";

export function useDeviceShadow(deviceId: string | null) {
  return useQuery({
    queryKey: queryKeys.shadow.detail(deviceId ?? ""),
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
      void queryClient.invalidateQueries({ queryKey: queryKeys.shadow.detail(variables.deviceId) });
    },
  });
}

export function useUpdateReportedState() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ deviceId, state }: { deviceId: string; state: Record<string, unknown> }) =>
      updateReportedState(deviceId, state),
    onSuccess: (_data, variables) => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.shadow.detail(variables.deviceId) });
    },
  });
}

export function useDeleteDeviceShadow() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (deviceId: string) => deleteDeviceShadow(deviceId),
    onSuccess: (_data, deviceId) => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.shadow.detail(deviceId) });
    },
  });
}
