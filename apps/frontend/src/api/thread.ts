import client from './client';
import type {
  CreateThreadNetworkRequest,
  ImportThreadDatasetRequest,
  ThreadStatus,
} from '../types/api';

export async function getThreadStatus(): Promise<ThreadStatus> {
  const { data } = await client.get<ThreadStatus>('/system/thread');
  return data;
}

export async function createThreadNetwork(body: CreateThreadNetworkRequest): Promise<ThreadStatus> {
  const { data } = await client.post<ThreadStatus>('/system/thread/network', body);
  return data;
}

export async function importThreadDataset(body: ImportThreadDatasetRequest): Promise<ThreadStatus> {
  const { data } = await client.put<ThreadStatus>('/system/thread/dataset', body);
  return data;
}
