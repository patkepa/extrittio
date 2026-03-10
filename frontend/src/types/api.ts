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

// ---------------------------------------------------------------------------
// Firmware Updates
// ---------------------------------------------------------------------------

export interface FirmwareUpdate {
  id: number;
  device_type_id: number;
  device_type_name: string;
  version: string;
  url: string;
  sha256: string | null;
  description: string | null;
  created_at: string;
  has_blob: boolean;
  file_size: number | null;
  filename: string | null;
}

export interface CreateFirmwareUpdateRequest {
  device_type_id: number;
  version?: string;
  url: string;
  sha256?: string;
  description?: string;
}

export interface FirmwareUpdatesParams {
  device_type_id?: number;
}

export interface NextVersionResponse {
  next_version: string;
}

export interface TriggerOtaRequest {
  firmware_update_id: number;
}

// ---------------------------------------------------------------------------
// OTA Deployments
// ---------------------------------------------------------------------------

export interface OtaDeployment {
  id: number;
  device_id: string;
  firmware_update_id: number;
  firmware_version: string;
  status: string;
  error_message: string | null;
  initiated_at: string;
  completed_at: string | null;
}

// ---------------------------------------------------------------------------
// Device Logs
// ---------------------------------------------------------------------------

export interface LogRecord {
  id: number;
  device_id: string;
  level: string;
  message: string;
  created_at: string;
}

export interface LogsParams {
  limit?: number;
  level?: string;
  since?: string;
}

// ---------------------------------------------------------------------------
// Device Configs
// ---------------------------------------------------------------------------

export interface DeviceConfigResponse {
  device_id: string;
  config: Record<string, unknown>;
  updated_at: string;
}

// ---------------------------------------------------------------------------
// Device Commands
// ---------------------------------------------------------------------------

export interface CommandRecord {
  id: string;
  device_id: string;
  command: string;
  params: Record<string, string>;
  status: 'sent' | 'delivered' | 'succeeded' | 'failed' | 'timed_out';
  response_payload: Record<string, unknown> | null;
  created_at: string;
  updated_at: string;
}

export interface SendCommandRequest {
  command: string;
  params?: Record<string, string>;
}

export interface CommandsParams {
  limit?: number;
  status?: string;
}

// ---------------------------------------------------------------------------
// Device Shadows
// ---------------------------------------------------------------------------

// Matches ShadowResponse in backend/src/api/shadows.rs
export interface DeviceShadow {
  device_id: string;
  desired: Record<string, unknown>;
  reported: Record<string, unknown>;
  delta: Record<string, unknown>;
  version: number;
  updated_at: string;
}
