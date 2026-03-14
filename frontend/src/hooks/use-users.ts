import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getUsers, createUser, deleteUser } from '../api/auth';
import type { CreateUserRequest } from '../types/api';
import { queryKeys } from './query-keys';

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
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.users.all });
    },
  });
}

export function useDeleteUser() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => deleteUser(id),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.users.all });
    },
  });
}
