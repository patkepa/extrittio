export const queryKeys = {
  devices: {
    all: ['devices'] as const,
    list: (params?: unknown) => ['devices', params] as const,
    fullList: (params?: unknown) => ['devices', 'all-pages', params] as const,
    detailAll: ['device'] as const,
    detail: (id: string) => ['device', id] as const,
  },
  dashboard: {
    stats: ['dashboard-stats'] as const,
  },
  telemetry: {
    list: (deviceId: string, params?: unknown) => ['telemetry', deviceId, params] as const,
    all: (deviceId: string) => ['telemetry-all', deviceId] as const,
  },
  logs: {
    list: (deviceId: string, params?: unknown) => ['device-logs', deviceId, params] as const,
  },
  commands: {
    list: (deviceId: string, params?: unknown) => ['command-history', deviceId, params] as const,
  },
  config: {
    detail: (deviceId: string) => ['device-config', deviceId] as const,
  },
  shadow: {
    detail: (deviceId: string) => ['device-shadow', deviceId] as const,
  },
  firmware: {
    all: ['firmware-updates'] as const,
    list: (params?: unknown) => ['firmware-updates', params] as const,
    nextVersionAll: ['firmware-next-version'] as const,
    nextVersion: (deviceTypeId: number) => ['firmware-next-version', deviceTypeId] as const,
    deployments: (deviceId: string) => ['ota-deployments', deviceId] as const,
    allDeployments: (params?: unknown) => ['ota-deployments', params] as const,
  },
  fleets: {
    all: ['fleets'] as const,
  },
  deviceTypes: {
    all: ['device-types'] as const,
  },
  users: {
    all: ['users'] as const,
  },
  certificates: {
    ca: ['ca-certificate'] as const,
    deviceStatusAll: ['device-certificate-status'] as const,
    deviceStatus: (id: string) => ['device-certificate-status', id] as const,
  },
  apiKeys: {
    all: ['api-keys'] as const,
  },
  rules: {
    all: ['rules'] as const,
    list: (params?: unknown) => ['rules', params] as const,
    detail: (id: string) => ['rule', id] as const,
  },
  alerts: {
    all: ['alerts'] as const,
    list: (params?: unknown) => ['alerts', params] as const,
    detail: (id: string) => ['alert', id] as const,
    summary: ['alerts', 'summary'] as const,
  },
  serverMetrics: {
    current: ['server-metrics-current'] as const,
    history: (params?: unknown) => ['server-metrics-history', params] as const,
  },
  zones: {
    all: ['zones'] as const,
    list: () => ['zones'] as const,
    detail: (id: string) => ['zone', id] as const,
  },
  locations: {
    latest: (deviceId: string) => ['device-location', deviceId, 'latest'] as const,
  },
} as const;
