import client from './client';
import type { ApiKey, CreateApiKeyRequest, CreateApiKeyResponse } from '../types/api';

export async function getApiKeys(): Promise<ApiKey[]> {
  const { data } = await client.get<ApiKey[]>('/api-keys');
  return data;
}

export async function createApiKey(body: CreateApiKeyRequest): Promise<CreateApiKeyResponse> {
  const { data } = await client.post<CreateApiKeyResponse>('/api-keys', body);
  return data;
}

export async function deleteApiKey(id: number): Promise<void> {
  await client.delete(`/api-keys/${id}`);
}
