export const queryKeys = {
  devices: {
    all: ["devices"] as const,
    list: (params?: unknown) => ["devices", params] as const,
    detailAll: ["device"] as const,
    detail: (id: string) => ["device", id] as const,
  },
  dashboard: {
    stats: ["dashboard-stats"] as const,
  },
  telemetry: {
    list: (deviceId: string, params?: unknown) =>
      ["telemetry", deviceId, params] as const,
  },
  logs: {
    list: (deviceId: string, params?: unknown) =>
      ["device-logs", deviceId, params] as const,
  },
  commands: {
    list: (deviceId: string, params?: unknown) =>
      ["command-history", deviceId, params] as const,
  },
  config: {
    detail: (deviceId: string) => ["device-config", deviceId] as const,
  },
  shadow: {
    detail: (deviceId: string) => ["device-shadow", deviceId] as const,
  },
  firmware: {
    all: ["firmware-updates"] as const,
    list: (params?: unknown) => ["firmware-updates", params] as const,
    nextVersionAll: ["firmware-next-version"] as const,
    nextVersion: (deviceTypeId: number) => ["firmware-next-version", deviceTypeId] as const,
    deployments: (deviceId: string) => ["ota-deployments", deviceId] as const,
  },
  fleets: {
    all: ["fleets"] as const,
  },
  deviceTypes: {
    all: ["device-types"] as const,
  },
  users: {
    all: ["users"] as const,
  },
} as const;
