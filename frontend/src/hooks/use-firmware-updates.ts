import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import type {
  CreateFirmwareUpdateRequest,
  FirmwareUpdatesParams,
  TriggerOtaRequest,
} from "../types/api";
import {
  getFirmwareUpdates,
  createFirmwareUpdate,
  deleteFirmwareUpdate,
  getNextVersion,
  getOtaDeployments,
  triggerOta,
  uploadFirmwareUpdate,
} from "../api/firmware-updates";
import type { UploadFirmwareRequest } from "../api/firmware-updates";

export function useFirmwareUpdates(params?: FirmwareUpdatesParams) {
  return useQuery({
    queryKey: ["firmware-updates", params],
    queryFn: () => getFirmwareUpdates(params),
    staleTime: 30_000,
  });
}

export function useNextVersion(deviceTypeId: number | null) {
  return useQuery({
    queryKey: ["firmware-next-version", deviceTypeId],
    queryFn: () => getNextVersion(deviceTypeId!),
    enabled: !!deviceTypeId,
    staleTime: 5_000,
  });
}

export function useOtaDeployments(deviceId: string) {
  return useQuery({
    queryKey: ["ota-deployments", deviceId],
    queryFn: () => getOtaDeployments(deviceId),
    staleTime: 10_000,
  });
}

export function useCreateFirmwareUpdate() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: CreateFirmwareUpdateRequest) =>
      createFirmwareUpdate(body),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["firmware-updates"] });
      void queryClient.invalidateQueries({
        queryKey: ["firmware-next-version"],
      });
    },
  });
}

export function useUploadFirmwareUpdate() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (req: UploadFirmwareRequest) => uploadFirmwareUpdate(req),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["firmware-updates"] });
      void queryClient.invalidateQueries({
        queryKey: ["firmware-next-version"],
      });
    },
  });
}

export function useDeleteFirmwareUpdate() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => deleteFirmwareUpdate(id),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["firmware-updates"] });
      void queryClient.invalidateQueries({
        queryKey: ["firmware-next-version"],
      });
    },
  });
}

export function useTriggerOta() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      deviceId,
      body,
    }: {
      deviceId: string;
      body: TriggerOtaRequest;
    }) => triggerOta(deviceId, body),
    onSuccess: (_data, variables) => {
      void queryClient.invalidateQueries({
        queryKey: ["device-shadow", variables.deviceId],
      });
      void queryClient.invalidateQueries({
        queryKey: ["ota-deployments", variables.deviceId],
      });
    },
  });
}
