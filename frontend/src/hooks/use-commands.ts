import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import type { SendCommandRequest, CommandsParams } from "../types/api";
import { sendCommand, getCommandHistory } from "../api/commands";
import { queryKeys } from "./query-keys";

export function useCommandHistory(deviceId: string | null, params?: CommandsParams) {
  return useQuery({
    queryKey: queryKeys.commands.list(deviceId ?? "", params),
    queryFn: () => getCommandHistory(deviceId!, params),
    enabled: !!deviceId,
    staleTime: 5_000,
    refetchInterval: 5_000,
  });
}

export function useSendCommand() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ deviceId, body }: { deviceId: string; body: SendCommandRequest }) =>
      sendCommand(deviceId, body),
    onSuccess: (_data, variables) => {
      void queryClient.invalidateQueries({
        queryKey: queryKeys.commands.list(variables.deviceId),
      });
    },
  });
}
