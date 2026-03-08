// ---------------------------------------------------------------------------
// Device Types
// ---------------------------------------------------------------------------

export interface DeviceType {
  id: number;
  name: string;
}

export interface CreateDeviceTypeRequest {
  name: string;
}

// ---------------------------------------------------------------------------
// Fleets
// ---------------------------------------------------------------------------

export interface Fleet {
  id: number;
  name: string;
  device_count: number;
}

export interface CreateFleetRequest {
  name: string;
}

// ---------------------------------------------------------------------------
// Devices
// ---------------------------------------------------------------------------

// Matches DeviceResponse in backend/src/api/devices.rs
export interface Device {
  id: string;
  name: string;
  device_type_id: number;
  device_type_name: string;
  fleet_id: number | null;
  fleet_name: string | null;
  status: "online" | "offline";
  last_seen: string;
  firmware: string;
  location: string;
  uptime: string;
}

// Matches NewDeviceRequest in backend/src/api/devices.rs
export interface CreateDeviceRequest {
  name: string;
  device_type_id: number;
  fleet_id?: number;
  location?: string;
  firmware?: string;
}

// Matches UpdateDeviceRequest in backend/src/api/devices.rs
export interface UpdateDeviceRequest {
  name?: string;
  device_type_id?: number;
  fleet_id?: number | null;
  location?: string;
  firmware?: string;
}

// Matches ListDevicesQuery in backend/src/api/devices.rs
export interface ListDevicesParams {
  status?: string;
  search?: string;
  fleet_id?: number;
}

// ---------------------------------------------------------------------------
// Telemetry
// ---------------------------------------------------------------------------

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

// Matches TelemetryQuery in backend/src/api/telemetry.rs
export interface TelemetryParams {
  limit?: number;
  since?: string;
}
