import { useQuery, useQueries, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  getCaCertificate,
  getDeviceCertificateStatus,
  regenerateDeviceCertificate,
} from '../api/certificates';
import { queryKeys } from './query-keys';

export function useCaCertificate() {
  return useQuery({
    queryKey: queryKeys.certificates.ca,
    queryFn: getCaCertificate,
    staleTime: 60_000,
    retry: false,
  });
}

export function useDeviceCertificateStatuses(deviceIds: string[]) {
  return useQueries({
    queries: deviceIds.map((id) => ({
      queryKey: queryKeys.certificates.deviceStatus(id),
      queryFn: () => getDeviceCertificateStatus(id),
      staleTime: 30_000,
    })),
  });
}

export function useRegenerateDeviceCertificate() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => regenerateDeviceCertificate(id),
    onSuccess: () => {
      void queryClient.invalidateQueries({
        queryKey: queryKeys.certificates.deviceStatusAll,
      });
    },
  });
}
