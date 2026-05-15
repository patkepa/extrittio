import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { createRole, deleteRole, getPermissions, getRoles, updateRole } from '../api/roles';
import { getMe } from '../api/auth';
import type { CreateRoleRequest, UpdateRoleRequest } from '../types/api';
import { queryKeys } from './query-keys';
import { useAuthStore } from '../stores/auth-store';

async function refreshCurrentUser() {
  if (!useAuthStore.getState().token) return;
  try {
    const me = await getMe();
    useAuthStore.getState().setUser(me);
  } catch {
    // The shared API client handles 401s by clearing the session.
  }
}

export function useRoles(options?: { enabled?: boolean }) {
  return useQuery({
    queryKey: queryKeys.roles.all,
    queryFn: getRoles,
    staleTime: 30_000,
    enabled: options?.enabled ?? true,
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
    onSuccess: async () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.roles.all });
      void queryClient.invalidateQueries({ queryKey: queryKeys.users.all });
      await refreshCurrentUser();
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
