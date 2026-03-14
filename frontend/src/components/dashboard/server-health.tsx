import { useMemo } from "react";
import { Card, Elevation, H5, ProgressBar, Tag } from "@blueprintjs/core";
import type { Intent } from "@blueprintjs/core";
import { MetricSparkline } from "./metric-sparkline";
import { useCurrentMetrics, useMetricsHistory } from "../../hooks/use-server-metrics";
import type { SystemMetricsSnapshot, AppMetricsSnapshot } from "../../types/api";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  const val = bytes / Math.pow(1024, i);
  return `${val.toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}

function formatRate(bytesPerInterval: number): string {
  const perSecond = bytesPerInterval / 10;
  return `${formatBytes(perSecond)}/s`;
}

function severityIntent(percent: number, memoryMode = false): Intent {
  const warnThreshold = memoryMode ? 85 : 80;
  if (percent >= 95) return "danger";
  if (percent >= warnThreshold) return "warning";
  return "success";
}

function severityColor(percent: number, memoryMode = false): string {
  const warnThreshold = memoryMode ? 85 : 80;
  if (percent >= 95) return "#DB3737";
  if (percent >= warnThreshold) return "#D99E0B";
  return "#0F9960";
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export const ServerHealth = () => {
  const { data: current } = useCurrentMetrics();

  const since = useMemo(
    () => new Date(Date.now() - 3600_000).toISOString().replace(/\.\d+Z$/, "Z"),
    []
  );
  const { data: history } = useMetricsHistory(since);

  const system = current?.system ?? null;
  const app = current?.app ?? null;
  const sysHistory = history?.system ?? [];
  const appHistory = history?.app ?? [];

  // If no data at all yet, show placeholder
  if (!system && !app) {
    return (
      <div className="server-health-section">
        <H5>Server Health</H5>
        <Card elevation={Elevation.ONE}>
          <p style={{ color: "var(--text-secondary, #8a9ba8)", padding: 16 }}>
            Collecting initial metrics...
          </p>
        </Card>
      </div>
    );
  }

  // Extract sparkline arrays from history
  const cpuSpark = sysHistory.map((s: SystemMetricsSnapshot) => s.cpu_usage_percent);
  const memSpark = sysHistory.map(
    (s: SystemMetricsSnapshot) =>
      s.memory_total_bytes > 0
        ? (s.memory_used_bytes / s.memory_total_bytes) * 100
        : 0,
  );
  const rxSpark = sysHistory.map((s: SystemMetricsSnapshot) => s.network_rx_bytes_delta);
  const txSpark = sysHistory.map((s: SystemMetricsSnapshot) => s.network_tx_bytes_delta);

  const reqSpark = appHistory.map((a: AppMetricsSnapshot) => a.request_count);
  const errSpark = appHistory.map((a: AppMetricsSnapshot) => a.error_count);
  const latSpark = appHistory.map((a: AppMetricsSnapshot) => a.p95_latency_ms);
  const zenInSpark = appHistory.map((a: AppMetricsSnapshot) => a.zenoh_messages_in);

  // Derived values
  const cpuPct = system?.cpu_usage_percent ?? 0;
  const memPct =
    system && system.memory_total_bytes > 0
      ? (system.memory_used_bytes / system.memory_total_bytes) * 100
      : 0;
  const diskPct =
    system && system.disk_total_bytes > 0
      ? (system.disk_used_bytes / system.disk_total_bytes) * 100
      : 0;
  const dbPoolTotal =
    app ? app.db_pool_active + app.db_pool_idle : 0;
  const dbPoolPct = dbPoolTotal > 0 ? ((app?.db_pool_active ?? 0) / dbPoolTotal) * 100 : 0;

  const showLoad =
    system &&
    (system.load_avg_1m !== 0 || system.load_avg_5m !== 0 || system.load_avg_15m !== 0);

  const avgLatency = app?.avg_latency_ms ?? 0;
  const errorCount = app?.error_count ?? 0;

  function healthStatus(value: number, warnAt: number, dangerAt: number): 'online' | 'warning' | 'offline' {
    if (value >= dangerAt) return 'offline';
    if (value >= warnAt) return 'warning';
    return 'online';
  }

  function latencyStatus(ms: number): 'online' | 'warning' | 'offline' {
    if (ms >= 500) return 'offline';
    if (ms >= 200) return 'warning';
    return 'online';
  }

  const healthMetrics = [
    { label: 'CPU', value: `${cpuPct.toFixed(1)}%`, status: healthStatus(cpuPct, 80, 95) },
    { label: 'MEMORY', value: `${memPct.toFixed(1)}%`, status: healthStatus(memPct, 85, 95) },
    { label: 'API LATENCY', value: `${avgLatency.toFixed(0)}ms`, status: latencyStatus(avgLatency) },
    { label: 'ERRORS', value: `${errorCount}`, status: errorCount > 0 ? 'offline' as const : 'online' as const },
  ];

  return (
    <div className="server-health-section">
      <H5>Server Health</H5>
      <div className="health-strip">
        {healthMetrics.map((metric) => (
          <div key={metric.label} className="health-metric">
            <span className={`status-led status-led--${metric.status}`} />
            <span className="health-label">{metric.label}</span>
            <span className="health-value mono-data">{metric.value}</span>
          </div>
        ))}
      </div>
      <div className="server-health-grid">
        {/* ---- System Resources ---- */}
        <Card elevation={Elevation.ONE} className="content-card">
          <div className="card-header">
            <H5>System Resources</H5>
          </div>
          <div className="server-metric-rows">
            {/* CPU */}
            <div className="server-metric-row">
              <span className="server-metric-label">CPU</span>
              <div className="server-metric-value">
                <Tag intent={severityIntent(cpuPct)} minimal>
                  {cpuPct.toFixed(1)}%
                </Tag>
              </div>
              <div className="server-metric-spark">
                <MetricSparkline data={cpuSpark} color={severityColor(cpuPct)} />
              </div>
            </div>

            {/* Memory */}
            <div className="server-metric-row">
              <span className="server-metric-label">Memory</span>
              <div className="server-metric-value">
                <Tag intent={severityIntent(memPct, true)} minimal>
                  {memPct.toFixed(1)}%
                </Tag>
                <span className="server-metric-detail">
                  {formatBytes(system?.memory_used_bytes ?? 0)} /{" "}
                  {formatBytes(system?.memory_total_bytes ?? 0)}
                </span>
              </div>
              <div className="server-metric-spark">
                <MetricSparkline data={memSpark} color={severityColor(memPct, true)} />
              </div>
            </div>

            {/* Disk */}
            <div className="server-metric-row">
              <span className="server-metric-label">Disk</span>
              <div className="server-metric-value">
                <Tag intent={severityIntent(diskPct)} minimal>
                  {diskPct.toFixed(1)}%
                </Tag>
                <span className="server-metric-detail">
                  {formatBytes(system?.disk_used_bytes ?? 0)} /{" "}
                  {formatBytes(system?.disk_total_bytes ?? 0)}
                </span>
                <ProgressBar
                  intent={severityIntent(diskPct)}
                  value={diskPct / 100}
                  stripes={false}
                  animate={false}
                />
              </div>
              <div className="server-metric-spark" />
            </div>

            {/* Network */}
            <div className="server-metric-row">
              <span className="server-metric-label">Network</span>
              <div className="server-metric-value">
                <span className="server-metric-detail">
                  RX {formatRate(system?.network_rx_bytes_delta ?? 0)} / TX{" "}
                  {formatRate(system?.network_tx_bytes_delta ?? 0)}
                </span>
              </div>
              <div className="server-metric-spark">
                <MetricSparkline data={rxSpark.length > 0 ? rxSpark : txSpark} color="#2965CC" />
              </div>
            </div>

            {/* Load Averages */}
            {showLoad && (
              <div className="server-metric-row">
                <span className="server-metric-label">Load</span>
                <div className="server-metric-value">
                  <span className="server-metric-detail">
                    {system!.load_avg_1m.toFixed(2)} / {system!.load_avg_5m.toFixed(2)} /{" "}
                    {system!.load_avg_15m.toFixed(2)}
                  </span>
                </div>
                <div className="server-metric-spark" />
              </div>
            )}
          </div>
        </Card>

        {/* ---- Application Performance ---- */}
        <Card elevation={Elevation.ONE} className="content-card">
          <div className="card-header">
            <H5>Application Performance</H5>
          </div>
          <div className="server-metric-rows">
            {/* Request Rate */}
            <div className="server-metric-row">
              <span className="server-metric-label">Requests</span>
              <div className="server-metric-value">
                <span className="server-metric-detail">
                  {((app?.request_count ?? 0) / 10).toFixed(1)} req/s
                </span>
              </div>
              <div className="server-metric-spark">
                <MetricSparkline data={reqSpark} color="#2965CC" />
              </div>
            </div>

            {/* Error Rate */}
            <div className="server-metric-row">
              <span className="server-metric-label">Errors</span>
              <div className="server-metric-value">
                <Tag
                  intent={(app?.error_count ?? 0) > 0 ? "danger" : "success"}
                  minimal
                >
                  {app?.error_count ?? 0}
                </Tag>
              </div>
              <div className="server-metric-spark">
                <MetricSparkline
                  data={errSpark}
                  color={(app?.error_count ?? 0) > 0 ? "#DB3737" : "#0F9960"}
                />
              </div>
            </div>

            {/* Latency */}
            <div className="server-metric-row">
              <span className="server-metric-label">Latency</span>
              <div className="server-metric-value">
                <span className="server-metric-detail">
                  avg {(app?.avg_latency_ms ?? 0).toFixed(1)}ms / p95{" "}
                  {(app?.p95_latency_ms ?? 0).toFixed(1)}ms
                </span>
              </div>
              <div className="server-metric-spark">
                <MetricSparkline data={latSpark} color="#D99E0B" />
              </div>
            </div>

            {/* DB Pool */}
            <div className="server-metric-row">
              <span className="server-metric-label">DB Pool</span>
              <div className="server-metric-value">
                <span className="server-metric-detail">
                  {app?.db_pool_active ?? 0} active / {app?.db_pool_idle ?? 0} idle
                </span>
                <ProgressBar
                  intent={severityIntent(dbPoolPct)}
                  value={dbPoolPct / 100}
                  stripes={false}
                  animate={false}
                />
              </div>
              <div className="server-metric-spark" />
            </div>

            {/* Zenoh Throughput */}
            <div className="server-metric-row">
              <span className="server-metric-label">Zenoh</span>
              <div className="server-metric-value">
                <span className="server-metric-detail">
                  in {((app?.zenoh_messages_in ?? 0) / 10).toFixed(1)}/s / out{" "}
                  {((app?.zenoh_messages_out ?? 0) / 10).toFixed(1)}/s
                </span>
              </div>
              <div className="server-metric-spark">
                <MetricSparkline data={zenInSpark} color="#8F398F" />
              </div>
            </div>
          </div>
        </Card>
      </div>
    </div>
  );
};
