import client from './client';
import type { Rule, CreateRuleRequest, UpdateRuleRequest } from '../types/rules';

export async function getRules(params?: Record<string, unknown>): Promise<Rule[]> {
  const { data } = await client.get<Rule[]>('/rules', { params });
  return data;
}

export async function getRule(id: string): Promise<Rule> {
  const { data } = await client.get<Rule>(`/rules/${id}`);
  return data;
}

export async function createRule(body: CreateRuleRequest): Promise<Rule> {
  const { data } = await client.post<Rule>('/rules', body);
  return data;
}

export async function updateRule(id: string, body: UpdateRuleRequest): Promise<Rule> {
  const { data } = await client.put<Rule>(`/rules/${id}`, body);
  return data;
}

export async function deleteRule(id: string): Promise<void> {
  await client.delete(`/rules/${id}`);
}

export async function toggleRule(id: string, enabled: boolean): Promise<void> {
  await client.put(`/rules/${id}/enabled`, { enabled });
}
