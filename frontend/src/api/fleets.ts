import client from "./client";
import type { Fleet, CreateFleetRequest } from "../types/api";

export async function getFleets(): Promise<Fleet[]> {
  const { data } = await client.get<{ data: Fleet[] }>("/fleets");
  return data.data;
}

export async function createFleet(body: CreateFleetRequest): Promise<Fleet> {
  const { data } = await client.post<Fleet>("/fleets", body);
  return data;
}

export async function updateFleet(id: number, body: { name: string }): Promise<Fleet> {
  const { data } = await client.patch<Fleet>(`/fleets/${id}`, body);
  return data;
}

export async function deleteFleet(id: number): Promise<void> {
  await client.delete(`/fleets/${id}`);
}
