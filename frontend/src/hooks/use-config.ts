import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getDeviceConfig, updateDeviceConfig } from '../api/configs';
import { queryKeys } from './query-keys';

export function useDeviceConfig(deviceId: string | null) {
  return useQuery({
    queryKey: queryKeys.config.detail(deviceId ?? ''),
    queryFn: () => getDeviceConfig(deviceId!),
    enabled: !!deviceId,
    staleTime: 30_000,
  });
}

export function useUpdateDeviceConfig() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ deviceId, config }: { deviceId: string; config: Record<string, unknown> }) =>
      updateDeviceConfig(deviceId, config),
    onSuccess: (_data, variables) => {
      void queryClient.invalidateQueries({
        queryKey: queryKeys.config.detail(variables.deviceId),
      });
    },
  });
}
