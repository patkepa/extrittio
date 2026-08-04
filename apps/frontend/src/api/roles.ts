import client from './client';
import type { CreateRoleRequest, Permission, Role, UpdateRoleRequest } from '../types/api';

export async function getRoles(): Promise<Role[]> {
  const { data } = await client.get<Role[]>('/roles');
  return data;
}

export async function getPermissions(): Promise<Permission[]> {
  const { data } = await client.get<Permission[]>('/roles/permissions');
  return data;
}

export async function createRole(body: CreateRoleRequest): Promise<Role> {
  const { data } = await client.post<Role>('/roles', body);
  return data;
}

export async function updateRole(id: number, body: UpdateRoleRequest): Promise<Role> {
  const { data } = await client.put<Role>(`/roles/${id}`, body);
  return data;
}

export async function deleteRole(id: number): Promise<void> {
  await client.delete(`/roles/${id}`);
}
