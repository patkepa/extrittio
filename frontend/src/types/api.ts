// Matches DeviceResponse in backend/src/api/devices.rs
export interface Device {
  id: string;
  name: string;
  type: string;
  status: "online" | "offline";
  last_seen: string;
  firmware: string;
  location: string;
  uptime: string;
}

// Matches TelemetryResponse in backend/src/api/telemetry.rs
export interface TelemetryRecord {
  id: number;
  device_id: string;
  temperature: number | null;
  humidity: number | null;
  battery_level: number | null;
  custom_json: string | null;
  received_at: string;
}

// Matches DashboardStats in backend/src/api/dashboard.rs
export interface DashboardStats {
  total_devices: number;
  active_devices: number;
  offline_devices: number;
  total_messages: number;
}

// Matches NewDeviceRequest in backend/src/api/devices.rs
export interface CreateDeviceRequest {
  name: string;
  device_type: string;
  location?: string;
  firmware?: string;
}

// Matches UpdateDeviceRequest in backend/src/api/devices.rs
export interface UpdateDeviceRequest {
  name?: string;
  device_type?: string;
  location?: string;
  firmware?: string;
}

// Matches ListDevicesQuery in backend/src/api/devices.rs
export interface ListDevicesParams {
  status?: string;
  search?: string;
}

// Matches TelemetryQuery in backend/src/api/telemetry.rs
export interface TelemetryParams {
  limit?: number;
  since?: string;
}
