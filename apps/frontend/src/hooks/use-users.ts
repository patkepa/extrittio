import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getUsers, createUser, deleteUser, setUserRoles, getMe } from '../api/auth';
import type { CreateUserRequest } from '../types/api';
import { queryKeys } from './query-keys';
import { useAuthStore } from '../stores/auth-store';

async function refreshCurrentUser() {
  try {
    const me = await getMe();
    useAuthStore.getState().setUser(me);
  } catch {
    // The shared API client handles 401s by clearing the session.
  }
}

export function useUsers() {
  return useQuery({
    queryKey: queryKeys.users.all,
    queryFn: getUsers,
    staleTime: 30_000,
  });
}

export function useCreateUser() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: CreateUserRequest) => createUser(body),
    onSuccess: async () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.users.all });
      await refreshCurrentUser();
    },
  });
}

export function useDeleteUser() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => deleteUser(id),
    onSuccess: async () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.users.all });
      await refreshCurrentUser();
    },
  });
}

export function useSetUserRoles() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, roleIds }: { id: number; roleIds: number[] }) => setUserRoles(id, roleIds),
    onSuccess: async () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.users.all });
      void queryClient.invalidateQueries({ queryKey: queryKeys.roles.all });
      await refreshCurrentUser();
    },
  });
}
