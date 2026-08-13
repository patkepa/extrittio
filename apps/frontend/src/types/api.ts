/**
 * Re-export auto-generated OpenAPI types under the names used throughout
 * the frontend.  The canonical definitions live in `openapi.ts` (generated
 * by `npm run generate-api`).  This file is the only place that maps
 * backend schema names → frontend aliases, so every other import can stay
 * unchanged.
 */

import type { components, operations } from './openapi';

// ---------------------------------------------------------------------------
// Auth
// ---------------------------------------------------------------------------

export type LoginRequest = components['schemas']['LoginRequest'];
export type LoginResponse = components['schemas']['LoginResponse'];
export interface RoleSummary {
  id: number;
  name: string;
  description?: string | null;
  is_system: boolean;
}
export type AuthUser = components['schemas']['UserResponse'] & {
  roles?: RoleSummary[];
  permissions?: string[];
  permission_version?: number;
};
export type CreateUserRequest = components['schemas']['CreateUserRequest'] & {
  role_ids?: number[];
};
export type ChangePasswordRequest = components['schemas']['ChangePasswordRequest'];
export interface SetUserRolesRequest {
  role_ids: number[];
}

export interface Role {
  id: number;
  name: string;
  description?: string | null;
  is_system: boolean;
  permissions: string[];
  user_count: number;
}

export interface Permission {
  key: string;
}

export interface CreateRoleRequest {
  name: string;
  description?: string | null;
  permissions: string[];
}

export interface UpdateRoleRequest {
  name?: string;
  description?: string | null;
  permissions?: string[];
}

// ---------------------------------------------------------------------------
// Dashboard
// ---------------------------------------------------------------------------

export type DashboardStats = components['schemas']['DashboardStats'];

// ---------------------------------------------------------------------------
// Devices
// ---------------------------------------------------------------------------

export type Device = components['schemas']['DeviceResponse'];
export type CreateDeviceRequest = components['schemas']['NewDeviceRequest'];
export type UpdateDeviceRequest = components['schemas']['UpdateDeviceRequest'];

export type ListDevicesParams = NonNullable<operations['list_devices']['parameters']['query']>;

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

export type DeviceType = components['schemas']['DeviceTypeResponse'];
export type CreateDeviceTypeRequest = components['schemas']['NewDeviceTypeRequest'];
export type UpdateDeviceTypeRequest = components['schemas']['UpdateDeviceTypeRequest'];

// ---------------------------------------------------------------------------
// Fleets
// ---------------------------------------------------------------------------

export type Fleet = components['schemas']['FleetResponse'];
export type CreateFleetRequest = components['schemas']['NewFleetRequest'];

// ---------------------------------------------------------------------------
// Shadows
// ---------------------------------------------------------------------------

export type DeviceShadow = components['schemas']['ShadowResponse'];

// ---------------------------------------------------------------------------
// Telemetry
// ---------------------------------------------------------------------------

export type TelemetryRecord = components['schemas']['TelemetryResponse'];

export type TelemetryParams = NonNullable<
  operations['get_device_telemetry']['parameters']['query']
>;

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

export type CommandRecord = components['schemas']['CommandResponse'];
export type SendCommandRequest = components['schemas']['SendCommandRequest'];

export type CommandsParams = NonNullable<operations['list_commands']['parameters']['query']>;

// ---------------------------------------------------------------------------
// Firmware Updates & OTA
// ---------------------------------------------------------------------------

export type FirmwareUpdate = components['schemas']['FirmwareUpdateResponse'] & {
  source?: string | null;
  commit_sha?: string | null;
  branch?: string | null;
  ci_run_url?: string | null;
  build_timestamp?: string | null;
  changelog?: string | null;
};
export type CreateFirmwareUpdateRequest = components['schemas']['NewFirmwareUpdateRequest'];
export type NextVersionResponse = components['schemas']['NextVersionResponse'];
export type TriggerOtaRequest = components['schemas']['TriggerOtaRequest'];
export type OtaDeployment = components['schemas']['OtaDeploymentResponse'];
export interface GlobalOtaDeployment {
  id: number;
  device_id: string;
  device_name: string;
  device_status: string;
  current_firmware: string;
  device_type_id: number;
  device_type_name: string;
  fleet_id: number | null;
  fleet_name: string | null;
  firmware_update_id: number;
  firmware_version: string;
  status: string;
  error_message: string | null;
  initiated_at: string;
  completed_at: string | null;
}

export interface OtaDeploymentsParams {
  status?: string;
  limit?: number;
  offset?: number;
}

export type FirmwareUpdatesParams = NonNullable<
  operations['list_firmware_updates']['parameters']['query']
>;

// ---------------------------------------------------------------------------
// Logs
// ---------------------------------------------------------------------------

export type LogRecord = components['schemas']['LogResponse'];

export type LogsParams = NonNullable<operations['get_device_logs']['parameters']['query']>;

// ---------------------------------------------------------------------------
// Device Config
// ---------------------------------------------------------------------------

export type DeviceConfigResponse = components['schemas']['ConfigResponse'];

// ---------------------------------------------------------------------------
// Certificates
// ---------------------------------------------------------------------------

export type CaCertificateResponse = components['schemas']['CaCertificateResponse'];
export type DeviceCertificateResponse = components['schemas']['DeviceCertificateResponse'];
export type DeviceCertificateStatusResponse =
  components['schemas']['DeviceCertificateStatusResponse'];

// ---------------------------------------------------------------------------
// Local Thread Border Router
// ---------------------------------------------------------------------------

export interface ThreadStatus {
  available: boolean;
  connected: boolean;
  error: string | null;
  rcp_device: string | null;
  available_rcp_devices: string[];
  role: string | null;
  network_name: string | null;
  channel: number | null;
  pan_id: string | null;
  extended_pan_id: string | null;
  mesh_local_prefix: string | null;
  addresses: string[];
}

export interface CreateThreadNetworkRequest {
  network_name: string;
  channel?: number;
  pan_id?: string;
  extended_pan_id?: string;
  network_key?: string;
}

export interface ImportThreadDatasetRequest {
  active_dataset_tlvs: string;
}

export interface ThreadNetwork {
  network_name: string | null;
  pan_id: string;
  extended_address: string;
  channel: number;
  rssi: number;
  lqi: number;
}

export interface ThreadChannelDiagnostics {
  channel: number;
  utilization_percent: number | null;
  max_rssi_dbm: number | null;
  network_count: number;
  strongest_network_rssi_dbm: number | null;
}

export interface ThreadRadioStatistics {
  cca_failure_rate_percent: number | null;
  latest_rssi_dbm: number | null;
  monitor_sample_count: number | null;
  tx_total: number | null;
  rx_total: number | null;
  tx_retries: number | null;
  tx_errors: number | null;
  rx_errors: number | null;
}

export interface ThreadNetworkDiagnostics {
  scanning: boolean;
  scanned_at: string | null;
  error: string | null;
  channels: ThreadChannelDiagnostics[];
  networks: ThreadNetwork[];
  devices: ThreadMeshDevice[];
  statistics: ThreadRadioStatistics;
  warnings: string[];
}

export interface ThreadMeshDevice {
  id: string;
  is_border_router: boolean;
  extended_address: string | null;
  mesh_local_eid_iid: string | null;
  omr_ipv6_addresses: string[];
  hostname: string | null;
  eui64: string | null;
  role: string | null;
  full_thread_device: boolean | null;
  rx_on_when_idle: boolean | null;
  full_network_data: boolean | null;
  rloc16: string | null;
  rloc_address: string | null;
  router_id: number | null;
  router_count: number | null;
  network_name: string | null;
  extended_pan_id: string | null;
  border_agent_id: string | null;
  border_agent_state: string | null;
  partition_id: number | null;
  leader_router_id: number | null;
  data_version: number | null;
  stable_data_version: number | null;
  created_at: string | null;
  updated_at: string | null;
}

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

export type HealthResponse = components['schemas']['HealthResponse'];
export type ReadyResponse = components['schemas']['ReadyResponse'];
export type ErrorBody = components['schemas']['ErrorBody'];

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

export interface LocationPoint {
  latitude: number;
  longitude: number;
  speed: number | null;
  altitude: number | null;
  heading: number | null;
  timestamp: string;
}
