import client from './client';
import type { DashboardStats } from '../types/api';

export async function getDashboardStats(): Promise<DashboardStats> {
  const { data } = await client.get<DashboardStats>('/dashboard/stats');
  return data;
}
