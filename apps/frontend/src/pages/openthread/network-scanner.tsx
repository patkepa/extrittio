import { useMemo, type CSSProperties, type ReactNode } from 'react';
import type { AxiosError } from 'axios';
import { Callout, Icon, Spinner, Tag } from '@blueprintjs/core';
import { useThreadScan, useThreadStatus } from '../../hooks/use-thread';
import type {
  ThreadChannelDiagnostics,
  ThreadNetwork,
  ThreadNetworkDiagnostics,
} from '../../types/api';
import {
  combinedErrorRate,
  energyLevel,
  percentageRate,
  recommendedChannel,
  signalCondition,
  utilizationCondition,
  type ConditionTone,
  type NetworkCondition,
} from './network-scanner-metrics';
import { isCurrentThreadNetwork } from './thread-mesh-graph-data';
import { ThreadScanStatus } from './thread-scan-status';

const INTEGER_FORMATTER = new Intl.NumberFormat();

type ChannelStyle = CSSProperties & {
  '--channel-utilization': string;
  '--channel-energy': string;
};

function apiErrorMessage(error: unknown): string {
  const axiosError = error as AxiosError<{ error?: string; message?: string }>;
  return (
    axiosError.response?.data?.error ??
    axiosError.response?.data?.message ??
    'Unable to scan the OpenThread radio environment.'
  );
}

function scanTimestamp(value: string | null | undefined, scanning: boolean): string {
  if (!value) return scanning ? 'Initial scan in progress' : 'Waiting for automatic scanning';
  const scannedAt = new Date(value);
  if (Number.isNaN(scannedAt.getTime())) return 'Latest radio observation';
  return `Last scan ${scannedAt.toLocaleTimeString()}`;
}

function latestRadioObservation(diagnostics: ThreadNetworkDiagnostics | undefined): string | null {
  const observations =
    diagnostics?.sources
      .filter((source) => source.source !== 'mesh_devices' && source.observed_at)
      .map((source) => source.observed_at as string)
      .sort() ?? [];
  return observations.at(-1) ?? diagnostics?.scanned_at ?? null;
}

function formatPercent(value: number | null | undefined, digits = 1): string {
  return value == null ? '—' : `${value.toFixed(digits)}%`;
}

function formatRssi(value: number | null | undefined): string {
  return value == null ? '—' : `${value} dBm`;
}

function formatCount(value: number | null | undefined): string {
  return value == null ? '—' : INTEGER_FORMATTER.format(value);
}

function conditionIntent(
  tone: ConditionTone,
): 'success' | 'primary' | 'warning' | 'danger' | 'none' {
  if (tone === 'excellent') return 'success';
  if (tone === 'good') return 'primary';
  if (tone === 'fair') return 'warning';
  if (tone === 'poor') return 'danger';
  return 'none';
}

export function NetworkScanner() {
  const statusQuery = useThreadStatus();
  const scanQuery = useThreadScan();
  const status = statusQuery.data;
  const diagnostics = scanQuery.data;
  const scanning = Boolean(diagnostics?.scanning);
  const radioObservedAt = latestRadioObservation(diagnostics);
  const activeChannel = useMemo(
    () => diagnostics?.channels.find((channel) => channel.channel === status?.channel) ?? null,
    [diagnostics?.channels, status?.channel],
  );
  const suggestedChannel = useMemo(
    () => recommendedChannel(diagnostics?.channels ?? []),
    [diagnostics?.channels],
  );
  const networkCondition = utilizationCondition(
    activeChannel?.utilization_percent,
    diagnostics?.statistics.cca_failure_rate_percent,
  );
  const signal = signalCondition(diagnostics?.statistics.latest_rssi_dbm);
  const scanError = scanQuery.error;
  const radioSourceError = diagnostics?.sources.find(
    (source) => source.source !== 'mesh_devices' && source.error,
  )?.error;
  const scanStatusError =
    diagnostics?.error ?? radioSourceError ?? (scanError ? apiErrorMessage(scanError) : null);

  return (
    <div className="network-scanner-view">
      <header className="network-scanner-toolbar">
        <div className="network-scanner-title">
          <Icon icon="satellite" size={18} />
          <div>
            <strong>Network Scanner</strong>
            <span>{scanTimestamp(radioObservedAt, scanning)}</span>
          </div>
        </div>
        <div className="network-scanner-toolbar-context">
          <span>
            Network <strong>{status?.network_name ?? '—'}</strong>
          </span>
          <span>
            Active channel <strong>{status?.channel ?? '—'}</strong>
          </span>
        </div>
        <ThreadScanStatus
          connected={Boolean(status?.connected)}
          scanning={scanning}
          scannedAt={radioObservedAt}
          error={scanStatusError}
        />
      </header>

      <main className="network-scanner-content">
        {statusQuery.isLoading ? (
          <ScannerEmptyState loading title="Checking OpenThread runtime" />
        ) : statusQuery.isError ? (
          <ScannerCallout intent="danger" title="OpenThread status is unavailable">
            The border-router status could not be loaded.
          </ScannerCallout>
        ) : !status?.available ? (
          <ScannerCallout intent="warning" title="Thread radio not detected">
            {status?.error ?? 'Connect a compatible Thread RCP dongle.'} Detection, connection, and
            scanning retry automatically.
          </ScannerCallout>
        ) : !status.connected ? (
          <ScannerCallout intent="warning" title="Thread network is not configured">
            {status.error ??
              'The Thread radio is ready. Create a new network or import an existing dataset to begin scanning.'}
          </ScannerCallout>
        ) : scanQuery.isLoading && !diagnostics ? (
          <ScannerEmptyState loading title="Loading the OpenThread scan" />
        ) : (scanError || diagnostics?.error) && !diagnostics?.scanned_at && !scanning ? (
          <ScannerCallout intent="danger" title="Network scan failed">
            {diagnostics?.error ?? apiErrorMessage(scanError)}
          </ScannerCallout>
        ) : scanning && !diagnostics?.scanned_at ? (
          <ScannerEmptyState
            loading
            title="Scanning the radio environment"
            description="Sampling channel energy, nearby Thread networks, mesh nodes, and radio counters…"
          />
        ) : diagnostics?.scanned_at ? (
          <ScannerResults
            diagnostics={diagnostics}
            activeChannel={activeChannel}
            activeChannelNumber={status.channel}
            suggestedChannel={suggestedChannel}
            networkCondition={networkCondition}
            signal={signal}
            scanning={scanning}
            currentNetwork={(network) => isCurrentThreadNetwork(network, status)}
          />
        ) : (
          <ScannerEmptyState
            title="Waiting for automatic scanning"
            description="Scanning starts when the Thread radio connects and refreshes about once a minute."
          />
        )}
      </main>
    </div>
  );
}

function ScannerResults({
  diagnostics,
  activeChannel,
  activeChannelNumber,
  suggestedChannel,
  networkCondition,
  signal,
  scanning,
  currentNetwork,
}: {
  diagnostics: ThreadNetworkDiagnostics;
  activeChannel: ThreadChannelDiagnostics | null;
  activeChannelNumber: number | null;
  suggestedChannel: number | null;
  networkCondition: NetworkCondition;
  signal: NetworkCondition;
  scanning: boolean;
  currentNetwork: (network: ThreadNetwork) => boolean;
}) {
  const statistics = diagnostics.statistics;
  const retryRate = percentageRate(statistics.tx_retries, statistics.tx_total);
  const errorRate = combinedErrorRate(statistics);

  return (
    <div className="network-scanner-results" aria-busy={scanning}>
      {diagnostics.error ? (
        <Callout intent="danger" icon="error" title="Latest refresh failed">
          The previous scan is still shown. {diagnostics.error}
        </Callout>
      ) : null}
      {diagnostics.warnings.map((warning) => (
        <Callout key={warning} intent="warning" icon="warning-sign">
          {warning}
        </Callout>
      ))}

      <section className="network-health-grid" aria-label="Network condition summary">
        <ConditionCard eyebrow="Network condition" condition={networkCondition} icon="pulse" />
        <ConditionCard
          eyebrow="Latest signal"
          condition={signal}
          icon="signal-search"
          value={formatRssi(statistics.latest_rssi_dbm)}
        />
        <MetricCard
          eyebrow="Active utilization"
          value={formatPercent(activeChannel?.utilization_percent)}
          detail={`Channel ${activeChannelNumber ?? '—'} occupancy`}
          icon="timeline-bar-chart"
        />
        <MetricCard
          eyebrow="CCA failures"
          value={formatPercent(statistics.cca_failure_rate_percent)}
          detail="Transmissions blocked by activity"
          icon="issue"
        />
        <MetricCard
          eyebrow="Quietest channel"
          value={suggestedChannel == null ? '—' : String(suggestedChannel)}
          detail={
            suggestedChannel === activeChannelNumber
              ? 'Current channel is the best observed'
              : 'Lowest observed radio pressure'
          }
          icon="endorsed"
        />
      </section>

      <section className="network-scanner-panel channel-conditions-panel">
        <div className="network-scanner-panel-heading">
          <div>
            <span>2.4 GHz spectrum</span>
            <h2>Channel conditions</h2>
          </div>
          <p>
            Utilization uses the long-running OpenThread monitor; energy is the peak RSSI from this
            scan.
          </p>
        </div>
        <ChannelChart
          channels={diagnostics.channels}
          activeChannel={activeChannelNumber}
          suggestedChannel={suggestedChannel}
        />
      </section>

      <div className="network-scanner-detail-grid">
        <section className="network-scanner-panel radio-statistics-panel">
          <div className="network-scanner-panel-heading">
            <div>
              <span>Border router</span>
              <h2>Radio statistics</h2>
            </div>
          </div>
          <dl className="radio-statistics-list">
            <Statistic
              label="Monitor samples"
              value={formatCount(statistics.monitor_sample_count)}
            />
            <Statistic label="Frames transmitted" value={formatCount(statistics.tx_total)} />
            <Statistic label="Frames received" value={formatCount(statistics.rx_total)} />
            <Statistic label="TX retries" value={formatCount(statistics.tx_retries)} />
            <Statistic label="Retry rate" value={formatPercent(retryRate)} />
            <Statistic label="Combined frame errors" value={formatPercent(errorRate)} />
          </dl>
        </section>

        <section className="network-scanner-panel nearby-networks-panel">
          <div className="network-scanner-panel-heading">
            <div>
              <span>Active scan</span>
              <h2>Nearby Thread networks</h2>
            </div>
            <Tag minimal round>
              {diagnostics.networks.length} detected
            </Tag>
          </div>
          <NearbyNetworks networks={diagnostics.networks} currentNetwork={currentNetwork} />
        </section>
      </div>
    </div>
  );
}

function ConditionCard({
  eyebrow,
  condition,
  icon,
  value,
}: {
  eyebrow: string;
  condition: NetworkCondition;
  icon: 'pulse' | 'signal-search';
  value?: string;
}) {
  return (
    <article className={`network-health-card condition-${condition.tone}`}>
      <div className="network-health-card-icon">
        <Icon icon={icon} size={17} />
      </div>
      <div>
        <span>{eyebrow}</span>
        <div className="network-health-card-value">
          <strong>{value ?? condition.label}</strong>
          <Tag minimal round intent={conditionIntent(condition.tone)}>
            {condition.label}
          </Tag>
        </div>
        <p>{condition.detail}</p>
      </div>
    </article>
  );
}

function MetricCard({
  eyebrow,
  value,
  detail,
  icon,
}: {
  eyebrow: string;
  value: string;
  detail: string;
  icon: 'timeline-bar-chart' | 'issue' | 'endorsed';
}) {
  return (
    <article className="network-health-card">
      <div className="network-health-card-icon">
        <Icon icon={icon} size={17} />
      </div>
      <div>
        <span>{eyebrow}</span>
        <div className="network-health-card-value">
          <strong>{value}</strong>
        </div>
        <p>{detail}</p>
      </div>
    </article>
  );
}

function ChannelChart({
  channels,
  activeChannel,
  suggestedChannel,
}: {
  channels: ThreadChannelDiagnostics[];
  activeChannel: number | null;
  suggestedChannel: number | null;
}) {
  return (
    <div className="channel-chart">
      <div className="channel-chart-scale" aria-hidden="true">
        <span>100%</span>
        <span>75%</span>
        <span>50%</span>
        <span>25%</span>
        <span>0%</span>
      </div>
      <div
        className="channel-chart-columns"
        role="img"
        aria-label="Utilization and peak energy for Thread channels 11 through 26"
      >
        {channels.map((channel) => {
          const utilization = channel.utilization_percent ?? 0;
          const tone = utilizationCondition(channel.utilization_percent).tone;
          const style: ChannelStyle = {
            '--channel-utilization': `${Math.min(Math.max(utilization, 0), 100)}%`,
            '--channel-energy': `${energyLevel(channel.max_rssi_dbm)}%`,
          };
          const isActive = channel.channel === activeChannel;
          const isSuggested = channel.channel === suggestedChannel;
          return (
            <div
              key={channel.channel}
              className={`channel-column condition-${tone}${isActive ? ' is-active' : ''}${isSuggested ? ' is-suggested' : ''}`}
              style={style}
              title={`Channel ${channel.channel}: ${formatPercent(channel.utilization_percent)} utilization, ${formatRssi(channel.max_rssi_dbm)} peak energy, ${channel.network_count} networks`}
            >
              <div className="channel-column-plot">
                <div className="channel-column-utilization" />
                {channel.max_rssi_dbm == null ? null : <i className="channel-energy-marker" />}
                <span className="channel-utilization-label">
                  {channel.utilization_percent == null
                    ? '—'
                    : `${Math.round(channel.utilization_percent)}%`}
                </span>
              </div>
              <strong>{channel.channel}</strong>
              <div className="channel-column-badges" aria-hidden="true">
                {isActive ? <i className="active-channel-badge">A</i> : null}
                {isSuggested ? <i className="suggested-channel-badge">Q</i> : null}
              </div>
            </div>
          );
        })}
      </div>
      <div className="channel-chart-legend">
        <span>
          <i className="utilization-legend" /> Channel utilization
        </span>
        <span>
          <i className="energy-legend" /> Peak energy
        </span>
        <span>
          <i className="active-legend">A</i> Active
        </span>
        <span>
          <i className="quiet-legend">Q</i> Quietest observed
        </span>
      </div>
    </div>
  );
}

function Statistic({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}

function NearbyNetworks({
  networks,
  currentNetwork,
}: {
  networks: ThreadNetwork[];
  currentNetwork: (network: ThreadNetwork) => boolean;
}) {
  if (networks.length === 0) {
    return (
      <div className="nearby-networks-empty">
        <Icon icon="search-around" size={30} />
        <p>No Thread beacons were detected during this scan.</p>
      </div>
    );
  }

  const sortedNetworks = [...networks].sort((left, right) => right.rssi - left.rssi);
  return (
    <div className="nearby-networks-table-wrap">
      <table className="nearby-networks-table">
        <thead>
          <tr>
            <th>Network</th>
            <th>Channel</th>
            <th>Signal</th>
            <th>LQI</th>
          </tr>
        </thead>
        <tbody>
          {sortedNetworks.map((network) => (
            <tr key={`${network.extended_address}-${network.pan_id}-${network.channel}`}>
              <td>
                <strong>{network.network_name || 'Unnamed network'}</strong>
                <span>{network.pan_id}</span>
                {currentNetwork(network) ? (
                  <Tag minimal round intent="primary">
                    Active
                  </Tag>
                ) : null}
              </td>
              <td>{network.channel}</td>
              <td>{formatRssi(network.rssi)}</td>
              <td>{network.lqi}/3</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function ScannerEmptyState({
  loading = false,
  title,
  description,
}: {
  loading?: boolean;
  title: string;
  description?: string;
}) {
  return (
    <div className="network-scanner-empty">
      {loading ? <Spinner size={34} /> : <Icon icon="satellite" size={48} />}
      <h2>{title}</h2>
      {description ? <p>{description}</p> : null}
    </div>
  );
}

function ScannerCallout({
  intent,
  title,
  children,
}: {
  intent: 'danger' | 'warning';
  title: string;
  children: ReactNode;
}) {
  return (
    <div className="network-scanner-callout">
      <Callout intent={intent} icon={intent === 'danger' ? 'error' : 'warning-sign'} title={title}>
        {children}
      </Callout>
    </div>
  );
}
