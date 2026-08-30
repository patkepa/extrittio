import client from './client';
import type {
  CreateThreadNetworkRequest,
  ConfigureThreadRuntimeRequest,
  ImportThreadDatasetRequest,
  ThreadDataset,
  ThreadNetworkDiagnostics,
  ThreadStatus,
} from '../types/api';

export async function getThreadStatus(): Promise<ThreadStatus> {
  const { data } = await client.get<ThreadStatus>('/system/thread');
  return data;
}

export async function getThreadScan(): Promise<ThreadNetworkDiagnostics> {
  const { data } = await client.get<ThreadNetworkDiagnostics>('/system/thread/scan');
  return data;
}

export async function getThreadDataset(): Promise<ThreadDataset> {
  const { data } = await client.get<ThreadDataset>('/system/thread/dataset');
  return data;
}

export async function configureThreadRuntime(
  body: ConfigureThreadRuntimeRequest,
): Promise<ThreadStatus> {
  const { data } = await client.put<ThreadStatus>('/system/thread/configuration', body);
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
