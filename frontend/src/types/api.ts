/**
 * Re-export auto-generated OpenAPI types under the names used throughout
 * the frontend.  The canonical definitions live in `openapi.ts` (generated
 * by `npm run generate-api`).  This file is the only place that maps
 * backend schema names → frontend aliases, so every other import can stay
 * unchanged.
 */

import type { components, operations } from "./openapi";

// ---------------------------------------------------------------------------
// Auth
// ---------------------------------------------------------------------------

export type LoginRequest = components["schemas"]["LoginRequest"];
export type LoginResponse = components["schemas"]["LoginResponse"];
export type AuthUser = components["schemas"]["UserResponse"];
export type CreateUserRequest = components["schemas"]["CreateUserRequest"];
export type ChangePasswordRequest = components["schemas"]["ChangePasswordRequest"];

// ---------------------------------------------------------------------------
// Dashboard
// ---------------------------------------------------------------------------

export type DashboardStats = components["schemas"]["DashboardStats"];

// ---------------------------------------------------------------------------
// Devices
// ---------------------------------------------------------------------------

export type Device = components["schemas"]["DeviceResponse"];
export type CreateDeviceRequest = components["schemas"]["NewDeviceRequest"];
export type UpdateDeviceRequest = components["schemas"]["UpdateDeviceRequest"];

export type ListDevicesParams = NonNullable<
  operations["list_devices"]["parameters"]["query"]
>;

// ---------------------------------------------------------------------------
// Bulk Operations
// ---------------------------------------------------------------------------

export interface BulkDeviceFilters {
  status?: string;
  search?: string;
  fleet_id?: number;
}

export interface BulkTargeting {
  device_ids?: string[];
  filters?: BulkDeviceFilters;
  select_all?: boolean;
}

export interface BulkFleetRequest extends BulkTargeting {
  fleet_id: number | null;
}

export interface BulkOtaRequest extends BulkTargeting {
  firmware_update_id: number;
}

export interface BulkAffectedResponse {
  affected: number;
}

export interface BulkOperationError {
  device_id: string;
  error: string;
}

export interface BulkResultResponse {
  succeeded: number;
  failed: number;
  errors: BulkOperationError[];
}

export interface PaginatedResponse<T> {
  data: T[];
  total: number;
  limit: number;
  offset: number;
}

// ---------------------------------------------------------------------------
// Device Types
// ---------------------------------------------------------------------------

export type DeviceType = components["schemas"]["DeviceTypeResponse"];
export type CreateDeviceTypeRequest = components["schemas"]["NewDeviceTypeRequest"];

// ---------------------------------------------------------------------------
// Fleets
// ---------------------------------------------------------------------------

export type Fleet = components["schemas"]["FleetResponse"];
export type CreateFleetRequest = components["schemas"]["NewFleetRequest"];

// ---------------------------------------------------------------------------
// Shadows
// ---------------------------------------------------------------------------

export type DeviceShadow = components["schemas"]["ShadowResponse"];

// ---------------------------------------------------------------------------
// Telemetry
// ---------------------------------------------------------------------------

export type TelemetryRecord = components["schemas"]["TelemetryResponse"];

export type TelemetryParams = NonNullable<
  operations["get_device_telemetry"]["parameters"]["query"]
>;

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

export type CommandRecord = components["schemas"]["CommandResponse"];
export type SendCommandRequest = components["schemas"]["SendCommandRequest"];

export type CommandsParams = NonNullable<
  operations["list_commands"]["parameters"]["query"]
>;

// ---------------------------------------------------------------------------
// Firmware Updates & OTA
// ---------------------------------------------------------------------------

export type FirmwareUpdate = components["schemas"]["FirmwareUpdateResponse"] & {
  source?: string | null;
  commit_sha?: string | null;
  branch?: string | null;
  ci_run_url?: string | null;
  build_timestamp?: string | null;
  changelog?: string | null;
};
export type CreateFirmwareUpdateRequest = components["schemas"]["NewFirmwareUpdateRequest"];
export type NextVersionResponse = components["schemas"]["NextVersionResponse"];
export type TriggerOtaRequest = components["schemas"]["TriggerOtaRequest"];
export type OtaDeployment = components["schemas"]["OtaDeploymentResponse"];

export type FirmwareUpdatesParams = NonNullable<
  operations["list_firmware_updates"]["parameters"]["query"]
>;

// ---------------------------------------------------------------------------
// Logs
// ---------------------------------------------------------------------------

export type LogRecord = components["schemas"]["LogResponse"];

export type LogsParams = NonNullable<
  operations["get_device_logs"]["parameters"]["query"]
>;

// ---------------------------------------------------------------------------
// Device Config
// ---------------------------------------------------------------------------

export type DeviceConfigResponse = components["schemas"]["ConfigResponse"];

// ---------------------------------------------------------------------------
// Certificates
// ---------------------------------------------------------------------------

export type CaCertificateResponse = components["schemas"]["CaCertificateResponse"];
export type DeviceCertificateResponse = components["schemas"]["DeviceCertificateResponse"];
export type DeviceCertificateStatusResponse = components["schemas"]["DeviceCertificateStatusResponse"];

// ---------------------------------------------------------------------------
// API Keys
// ---------------------------------------------------------------------------

export interface ApiKey {
  id: number;
  name: string;
  key_prefix: string;
  device_type_id: number | null;
  device_type_name: string | null;
  created_at: string;
  last_used_at: string | null;
}

export interface CreateApiKeyRequest {
  name: string;
  device_type_id?: number;
}

export interface CreateApiKeyResponse {
  id: number;
  name: string;
  key: string;
  key_prefix: string;
  device_type_id: number | null;
}

// ---------------------------------------------------------------------------
// Health
// ---------------------------------------------------------------------------

export type HealthResponse = components["schemas"]["HealthResponse"];
export type ReadyResponse = components["schemas"]["ReadyResponse"];
export type ErrorBody = components["schemas"]["ErrorBody"];

// ---------------------------------------------------------------------------
// Server Metrics
// ---------------------------------------------------------------------------

export interface SystemMetricsSnapshot {
  cpu_usage_percent: number;
  memory_used_bytes: number;
  memory_total_bytes: number;
  disk_used_bytes: number;
  disk_total_bytes: number;
  network_rx_bytes_delta: number;
  network_tx_bytes_delta: number;
  load_avg_1m: number;
  load_avg_5m: number;
  load_avg_15m: number;
  recorded_at: string;
}

export interface AppMetricsSnapshot {
  request_count: number;
  error_count: number;
  avg_latency_ms: number;
  p95_latency_ms: number;
  db_pool_active: number;
  db_pool_idle: number;
  zenoh_messages_in: number;
  zenoh_messages_out: number;
  recorded_at: string;
}

export interface CurrentMetricsResponse {
  system: SystemMetricsSnapshot | null;
  app: AppMetricsSnapshot | null;
}

export interface MetricsHistoryResponse {
  system: SystemMetricsSnapshot[];
  app: AppMetricsSnapshot[];
}
