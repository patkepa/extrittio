import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { createRole, deleteRole, getPermissions, getRoles, updateRole } from '../api/roles';
import type { CreateRoleRequest, UpdateRoleRequest } from '../types/api';
import { queryKeys } from './query-keys';

export function useRoles() {
  return useQuery({
    queryKey: queryKeys.roles.all,
    queryFn: getRoles,
    staleTime: 30_000,
  });
}

export function usePermissions() {
  return useQuery({
    queryKey: queryKeys.roles.permissions,
    queryFn: getPermissions,
    staleTime: 60_000,
  });
}

export function useCreateRole() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: CreateRoleRequest) => createRole(body),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.roles.all });
    },
  });
}

export function useUpdateRole() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, body }: { id: number; body: UpdateRoleRequest }) => updateRole(id, body),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.roles.all });
      void queryClient.invalidateQueries({ queryKey: queryKeys.users.all });
    },
  });
}

export function useDeleteRole() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => deleteRole(id),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.roles.all });
    },
  });
}
