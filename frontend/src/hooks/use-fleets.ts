import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import type { CreateFleetRequest } from "../types/api";
import { getFleets, createFleet, deleteFleet } from "../api/fleets";

export function useFleets() {
  return useQuery({
    queryKey: ["fleets"],
    queryFn: getFleets,
    staleTime: 60_000,
  });
}

export function useCreateFleet() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: CreateFleetRequest) => createFleet(body),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["fleets"] });
    },
  });
}

export function useDeleteFleet() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => deleteFleet(id),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["fleets"] });
      void queryClient.invalidateQueries({ queryKey: ["devices"] });
    },
  });
}
