import client from '../../../api/client';
import type { ActivityEventPage, ActivityEventsParams } from '../../../types/api';

export async function getActivityEvents(params?: ActivityEventsParams): Promise<ActivityEventPage> {
  const { data } = await client.get<ActivityEventPage>('/activity-events', { params });
  return data;
}
