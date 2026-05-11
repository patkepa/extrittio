export interface Zone {
  id: string;
  name: string;
  description: string;
  geometry_type: 'circle' | 'polygon';
  geometry_json: CircleGeometry | PolygonGeometry;
  color: string;
  created_at: string;
  updated_at: string;
}

export interface CircleGeometry {
  center: [number, number];
  radius_meters: number;
}

export interface PolygonGeometry {
  points: [number, number][];
}

export interface CreateZoneRequest {
  name: string;
  description?: string;
  geometry_type: 'circle' | 'polygon';
  geometry_json: CircleGeometry | PolygonGeometry;
  color?: string;
}

export type UpdateZoneRequest = CreateZoneRequest;
