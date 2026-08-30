import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { DeviceBlueprintDocument } from '../types/api';
import {
  createDeviceBlueprint,
  getDeviceBlueprintDraft,
  getDeviceBlueprints,
  getLatestDeviceBlueprintRevision,
  publishDeviceBlueprintDraft,
  replaceDeviceBlueprintDraft,
  validateDeviceBlueprintDraft,
} from '../api/device-blueprints';
import { queryKeys } from './query-keys';

export function useDeviceBlueprints() {
  return useQuery({
    queryKey: queryKeys.deviceBlueprints.all,
    queryFn: getDeviceBlueprints,
    staleTime: 60_000,
  });
}

export function useLatestDeviceBlueprintRevision(blueprintId: string) {
  return useQuery({
    queryKey: queryKeys.deviceBlueprints.latestRevision(blueprintId),
    queryFn: () => getLatestDeviceBlueprintRevision(blueprintId),
    enabled: blueprintId.length > 0,
    staleTime: 60_000,
  });
}

export function useDeviceBlueprintDraft(blueprintId: string) {
  return useQuery({
    queryKey: queryKeys.deviceBlueprints.draft(blueprintId),
    queryFn: () => getDeviceBlueprintDraft(blueprintId),
    enabled: blueprintId.length > 0,
  });
}

export function useCreateDeviceBlueprint() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (document: DeviceBlueprintDocument) => createDeviceBlueprint(document),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.deviceBlueprints.all });
    },
  });
}

export function useReplaceDeviceBlueprintDraft() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      blueprintId,
      document,
    }: {
      blueprintId: string;
      document: DeviceBlueprintDocument;
    }) => replaceDeviceBlueprintDraft(blueprintId, document),
    onSuccess: (draft) => {
      queryClient.setQueryData(queryKeys.deviceBlueprints.draft(draft.blueprint_id), draft);
      void queryClient.invalidateQueries({ queryKey: queryKeys.deviceBlueprints.all });
    },
  });
}

export function useValidateDeviceBlueprintDraft() {
  return useMutation({ mutationFn: validateDeviceBlueprintDraft });
}

export function usePublishDeviceBlueprintDraft() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: publishDeviceBlueprintDraft,
    onSuccess: (revision) => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.deviceBlueprints.all });
      queryClient.setQueryData(
        queryKeys.deviceBlueprints.latestRevision(revision.blueprint_id),
        revision,
      );
    },
  });
}
