import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { getZones, getZone, createZone, updateZone, deleteZone } from "../api/zones";
import { queryKeys } from "./query-keys";
import type { CreateZoneRequest, UpdateZoneRequest } from "../types/zones";

export function useZones() {
  return useQuery({
    queryKey: queryKeys.zones.list(),
    queryFn: getZones,
    staleTime: 30_000,
  });
}

export function useZone(id: string | null) {
  return useQuery({
    queryKey: queryKeys.zones.detail(id!),
    queryFn: () => getZone(id!),
    enabled: !!id,
  });
}

export function useCreateZone() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: CreateZoneRequest) => createZone(body),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: queryKeys.zones.all }),
  });
}

export function useUpdateZone() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, body }: { id: string; body: UpdateZoneRequest }) => updateZone(id, body),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: queryKeys.zones.all }),
  });
}

export function useDeleteZone() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => deleteZone(id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: queryKeys.zones.all }),
  });
}
