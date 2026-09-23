import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import type {
  CreateFirmwareUpdateRequest,
  FirmwareUpdatesParams,
  OtaDeploymentsParams,
  TriggerOtaRequest,
} from '../types/api';
import {
  getFirmwareUpdates,
  getAllOtaDeployments,
  createFirmwareUpdate,
  deleteFirmwareUpdate,
  getNextBlueprintVersion,
  getOtaDeployments,
  triggerOta,
  uploadFirmwareUpdate,
} from '../api/firmware-updates';
import type { UploadFirmwareRequest } from '../api/firmware-updates';
import { queryKeys } from './query-keys';

export function useFirmwareUpdates(
  params?: FirmwareUpdatesParams,
  options?: { enabled?: boolean },
) {
  return useQuery({
    queryKey: queryKeys.firmware.list(params),
    queryFn: () => getFirmwareUpdates(params),
    enabled: options?.enabled ?? true,
    staleTime: 30_000,
  });
}

export function useNextBlueprintVersion(revisionId: string | null) {
  return useQuery({
    queryKey: queryKeys.firmware.nextBlueprintVersion(revisionId ?? ''),
    queryFn: () => getNextBlueprintVersion(revisionId!),
    enabled: !!revisionId,
    staleTime: 5_000,
  });
}

export function useOtaDeployments(deviceId: string | null) {
  return useQuery({
    queryKey: queryKeys.firmware.deployments(deviceId ?? ''),
    queryFn: () => getOtaDeployments(deviceId!),
    enabled: !!deviceId,
    staleTime: 10_000,
  });
}

export function useAllOtaDeployments(params?: OtaDeploymentsParams) {
  return useQuery({
    queryKey: queryKeys.firmware.allDeployments(params),
    queryFn: () => getAllOtaDeployments(params),
    staleTime: 5_000,
    refetchInterval: params?.status === 'in_progress' ? 10_000 : false,
  });
}

export function useCreateFirmwareUpdate() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: CreateFirmwareUpdateRequest) => createFirmwareUpdate(body),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.firmware.all });
      void queryClient.invalidateQueries({
        queryKey: queryKeys.firmware.nextBlueprintVersionAll,
      });
    },
  });
}

export function useUploadFirmwareUpdate() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (req: UploadFirmwareRequest) => uploadFirmwareUpdate(req),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.firmware.all });
      void queryClient.invalidateQueries({
        queryKey: queryKeys.firmware.nextBlueprintVersionAll,
      });
    },
  });
}

export function useDeleteFirmwareUpdate() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => deleteFirmwareUpdate(id),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.firmware.all });
      void queryClient.invalidateQueries({
        queryKey: queryKeys.firmware.nextBlueprintVersionAll,
      });
    },
  });
}

export function useTriggerOta() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ deviceId, body }: { deviceId: string; body: TriggerOtaRequest }) =>
      triggerOta(deviceId, body),
    onSuccess: (_data, variables) => {
      void queryClient.invalidateQueries({
        queryKey: queryKeys.shadow.detail(variables.deviceId),
      });
      void queryClient.invalidateQueries({
        queryKey: queryKeys.firmware.deployments(variables.deviceId),
      });
    },
  });
}
