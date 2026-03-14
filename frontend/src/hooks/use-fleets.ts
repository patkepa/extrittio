import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import type { CreateFleetRequest } from "../types/api";
import { getFleets, createFleet, updateFleet, deleteFleet } from "../api/fleets";
import { queryKeys } from "./query-keys";

export function useFleets() {
  return useQuery({
    queryKey: queryKeys.fleets.all,
    queryFn: getFleets,
    staleTime: 60_000,
  });
}

export function useCreateFleet() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: CreateFleetRequest) => createFleet(body),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.fleets.all });
    },
  });
}

export function useUpdateFleet() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, name }: { id: number; name: string }) =>
      updateFleet(id, { name }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.fleets.all });
    },
  });
}

export function useDeleteFleet() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => deleteFleet(id),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.fleets.all });
      void queryClient.invalidateQueries({ queryKey: queryKeys.devices.all });
    },
  });
}
