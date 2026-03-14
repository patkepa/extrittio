import client from './client';
import type { LoginRequest, LoginResponse, AuthUser, CreateUserRequest } from '../types/api';

export async function login(body: LoginRequest): Promise<LoginResponse> {
  const { data } = await client.post<LoginResponse>('/auth/login', body);
  return data;
}

export async function getMe(): Promise<AuthUser> {
  const { data } = await client.get<AuthUser>('/auth/me');
  return data;
}

export async function getUsers(): Promise<AuthUser[]> {
  const { data } = await client.get<{ data: AuthUser[] }>('/users');
  return data.data;
}

export async function createUser(body: CreateUserRequest): Promise<AuthUser> {
  const { data } = await client.post<AuthUser>('/users', body);
  return data;
}

export async function deleteUser(id: number): Promise<void> {
  await client.delete(`/users/${id}`);
}

export async function changePassword(id: number, password: string): Promise<void> {
  await client.put(`/users/${id}/password`, { password });
}
