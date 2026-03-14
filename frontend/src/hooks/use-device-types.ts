import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import type { CreateDeviceTypeRequest } from '../types/api';
import { getDeviceTypes, createDeviceType, deleteDeviceType } from '../api/device-types';
import { queryKeys } from './query-keys';

export function useDeviceTypes() {
  return useQuery({
    queryKey: queryKeys.deviceTypes.all,
    queryFn: getDeviceTypes,
    staleTime: 60_000,
  });
}

export function useCreateDeviceType() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: CreateDeviceTypeRequest) => createDeviceType(body),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.deviceTypes.all });
    },
  });
}

export function useDeleteDeviceType() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => deleteDeviceType(id),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.deviceTypes.all });
    },
  });
}
