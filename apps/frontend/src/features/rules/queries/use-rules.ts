import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import type { CreateRuleRequest, UpdateRuleRequest } from '../../../types/rules';
import { getRules, getRule, createRule, updateRule, deleteRule, toggleRule } from '../api/rules';
import { queryKeys } from '../../../hooks/query-keys';

export function useRules(params?: Record<string, unknown>) {
  return useQuery({
    queryKey: queryKeys.rules.list(params),
    queryFn: () => getRules(params),
    staleTime: 30_000,
  });
}

export function useRule(id: string | null) {
  return useQuery({
    queryKey: queryKeys.rules.detail(id ?? ''),
    queryFn: () => getRule(id!),
    enabled: !!id,
  });
}

export function useCreateRule() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: CreateRuleRequest) => createRule(body),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.rules.all });
    },
  });
}

export function useUpdateRule() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, body }: { id: string; body: UpdateRuleRequest }) => updateRule(id, body),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.rules.all });
    },
  });
}

export function useDeleteRule() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => deleteRule(id),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.rules.all });
    },
  });
}

export function useToggleRule() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, enabled }: { id: string; enabled: boolean }) => toggleRule(id, enabled),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.rules.all });
    },
  });
}
