import { useState } from 'react';
import {
  Alert,
  Button,
  Callout,
  Card,
  Elevation,
  FormGroup,
  H3,
  HTMLTable,
  HTMLSelect,
  InputGroup,
  Spinner,
  Tag,
} from '@blueprintjs/core';
import {
  useCreateThreadNetwork,
  useImportThreadDataset,
  useThreadNetworkScan,
  useThreadRuntimeRefresh,
  useThreadStatus,
} from '../../hooks/use-thread';
import type { ThreadNetwork, ThreadStatus } from '../../types/api';
import { showErrorToast, showSuccessToast } from '../../utils/toaster';
import './settings.css';
import './thread.css';

const CHANNELS = Array.from({ length: 16 }, (_, index) => index + 11);

export function ThreadSettings() {
  const [networkName, setNetworkName] = useState('Extrittio-Thread');
  const [channel, setChannel] = useState('15');
  const [panId, setPanId] = useState('');
  const [extendedPanId, setExtendedPanId] = useState('');
  const [networkKey, setNetworkKey] = useState('');
  const [dataset, setDataset] = useState('');
  const [pendingAction, setPendingAction] = useState<'create' | 'import' | null>(null);

  const statusQuery = useThreadStatus();
  const createMutation = useCreateThreadNetwork();
  const importMutation = useImportThreadDataset();
  const scanMutation = useThreadNetworkScan();
  const refreshMutation = useThreadRuntimeRefresh();
  const status = statusQuery.data;

  const submit = () => {
    if (pendingAction === 'create') {
      createMutation.mutate(
        {
          network_name: networkName.trim(),
          channel: Number(channel),
          pan_id: optionalValue(panId),
          extended_pan_id: optionalValue(extendedPanId),
          network_key: optionalValue(networkKey),
        },
        {
          onSuccess: () => {
            setNetworkKey('');
            setPendingAction(null);
            void showSuccessToast('Thread network created');
          },
          onError: () => {
            void showErrorToast('Unable to create the Thread network');
          },
        },
      );
      return;
    }

    if (pendingAction === 'import') {
      importMutation.mutate(
        { active_dataset_tlvs: dataset.trim() },
        {
          onSuccess: () => {
            setDataset('');
            setPendingAction(null);
            void showSuccessToast('Thread dataset imported');
          },
          onError: () => {
            void showErrorToast('Unable to import the Thread dataset');
          },
        },
      );
    }
  };

  const busy = createMutation.isPending || importMutation.isPending;
  const scan = scanMutation.data;

  return (
    <div className="settings-page">
      <div className="page-header">
        <div>
          <H3>Thread Network</H3>
          <p className="page-description">
            Configure the local OpenThread border router used by this hobby appliance.
          </p>
        </div>
        <Button
          icon="refresh"
          minimal
          loading={statusQuery.isFetching || refreshMutation.isPending}
          onClick={() => {
            refreshMutation.mutate(undefined, {
              onError: () => {
                void showErrorToast('Unable to refresh the Thread runtime');
              },
            });
          }}
        >
          Refresh
        </Button>
      </div>

      {statusQuery.isLoading ? (
        <Spinner size={24} />
      ) : statusQuery.isError ? (
        <Callout intent="danger" icon="error">
          Unable to load the Thread border-router status.
        </Callout>
      ) : !status?.available ? (
        <Callout intent="warning" icon="warning-sign">
          {status?.error ??
            'No controllable OpenThread border router is running. Connect an RCP and refresh this page.'}
        </Callout>
      ) : (
        <div className="settings-content">
          <Card elevation={Elevation.ONE} className="settings-card">
            <div className="thread-status-heading">
              <span className="section-label">Border Router Status</span>
              <Tag intent={status.connected ? 'success' : 'danger'} minimal>
                {status.connected ? 'Connected' : 'Unavailable'}
              </Tag>
            </div>
            {status.connected ? <ThreadStatusDetails status={status} /> : null}
            {status.error ? <Callout intent="danger">{status.error}</Callout> : null}
          </Card>

          <Card elevation={Elevation.ONE} className="settings-card">
            <div className="thread-status-heading">
              <span className="section-label">Nearby Thread Networks</span>
              <Button
                icon="search"
                loading={scanMutation.isPending}
                disabled={!status.connected}
                onClick={() => {
                  scanMutation.mutate(undefined, {
                    onError: () => {
                      void showErrorToast('Unable to scan for Thread networks');
                    },
                  });
                }}
              >
                Scan Local Radio
              </Button>
            </div>
            <p className="thread-help">
              Scan with this border router&apos;s local radio to find nearby Thread networks. A scan
              reveals PAN, MAC address, channel, and signal only; you still need an Active
              Operational Dataset to join.
            </p>
            {scan ? <ThreadNetworkList networks={scan.networks} /> : null}
          </Card>

          <Card elevation={Elevation.ONE} className="settings-card">
            <span className="section-label">Create a New Thread Network</span>
            <p className="thread-help">
              Form a new mesh on this border router. Leave identifiers and the network key empty to
              let OpenThread generate them securely.
            </p>
            <div className="settings-form thread-form">
              <FormGroup
                label="Network name"
                labelFor="thread-network-name"
                helperText="1–16 ASCII characters"
              >
                <InputGroup
                  id="thread-network-name"
                  maxLength={16}
                  value={networkName}
                  onChange={(event) => setNetworkName(event.target.value)}
                />
              </FormGroup>
              <FormGroup label="Channel" labelFor="thread-channel">
                <HTMLSelect
                  id="thread-channel"
                  value={channel}
                  onChange={(event) => setChannel(event.target.value)}
                  options={CHANNELS.map((value) => ({
                    label: String(value),
                    value: String(value),
                  }))}
                />
              </FormGroup>
              <FormGroup
                label="PAN ID"
                labelFor="thread-pan-id"
                helperText="Optional 4 hexadecimal characters"
              >
                <InputGroup
                  id="thread-pan-id"
                  placeholder="e.g. 1234"
                  value={panId}
                  onChange={(event) => setPanId(event.target.value)}
                />
              </FormGroup>
              <FormGroup
                label="Extended PAN ID"
                labelFor="thread-extended-pan-id"
                helperText="Optional 16 hexadecimal characters"
              >
                <InputGroup
                  id="thread-extended-pan-id"
                  placeholder="e.g. 0011223344556677"
                  value={extendedPanId}
                  onChange={(event) => setExtendedPanId(event.target.value)}
                />
              </FormGroup>
              <FormGroup
                label="Network key"
                labelFor="thread-network-key"
                helperText="Optional 32 hexadecimal characters. Never shown again."
              >
                <InputGroup
                  id="thread-network-key"
                  type="password"
                  placeholder="Leave empty to generate"
                  value={networkKey}
                  onChange={(event) => setNetworkKey(event.target.value)}
                />
              </FormGroup>
            </div>
            <Button
              intent="primary"
              icon="add"
              disabled={busy || !networkName.trim()}
              onClick={() => setPendingAction('create')}
            >
              Create Network
            </Button>
          </Card>

          <Card elevation={Elevation.ONE} className="settings-card">
            <span className="section-label">Connect to an Existing Network</span>
            <p className="thread-help">
              Import the complete Active Operational Dataset exported from the existing Thread
              network. The dataset is write-only and is not saved by Extrittio.
            </p>
            <FormGroup label="Active Operational Dataset (hex TLVs)" labelFor="thread-dataset">
              <InputGroup
                id="thread-dataset"
                type="password"
                placeholder="Paste the exported Thread dataset"
                value={dataset}
                onChange={(event) => setDataset(event.target.value)}
              />
            </FormGroup>
            <Button
              intent="primary"
              icon="import"
              disabled={busy || !dataset.trim()}
              onClick={() => setPendingAction('import')}
            >
              Import Dataset
            </Button>
          </Card>
        </div>
      )}

      <Alert
        cancelButtonText="Cancel"
        confirmButtonText={
          pendingAction === 'create' ? 'Create and replace network' : 'Import and replace network'
        }
        icon="warning-sign"
        intent="danger"
        isOpen={pendingAction !== null}
        loading={busy}
        onCancel={() => setPendingAction(null)}
        onConfirm={submit}
      >
        This replaces the active Thread operational dataset. Existing Thread devices will disconnect
        until they are configured for the new network.
      </Alert>
    </div>
  );
}

function ThreadNetworkList({ networks }: { networks: ThreadNetwork[] }) {
  if (networks.length === 0) {
    return <Callout icon="info-sign">No Thread networks were discovered by the local radio.</Callout>;
  }

  return (
    <div className="thread-scan-results">
      <HTMLTable compact striped interactive>
        <thead>
          <tr>
            <th>PAN ID</th>
            <th>MAC address</th>
            <th>Channel</th>
            <th>Signal</th>
          </tr>
        </thead>
        <tbody>
          {networks.map((network) => (
            <tr key={`${network.extended_address}-${network.pan_id}-${network.channel}`}>
              <td>{network.pan_id}</td>
              <td className="thread-network-address">{network.extended_address}</td>
              <td>{network.channel}</td>
              <td>
                {network.rssi} dBm · LQI {network.lqi}
              </td>
            </tr>
          ))}
        </tbody>
      </HTMLTable>
    </div>
  );
}

function ThreadStatusDetails({ status }: { status: ThreadStatus }) {
  const rows = [
    ['Role', status.role],
    ['Network', status.network_name],
    ['Channel', status.channel?.toString()],
    ['PAN ID', status.pan_id],
    ['Extended PAN ID', status.extended_pan_id],
    ['Mesh-local prefix', status.mesh_local_prefix],
  ].filter(([, value]) => value);

  return (
    <dl className="thread-status-grid">
      {rows.map(([label, value]) => (
        <div key={label}>
          <dt>{label}</dt>
          <dd>{value}</dd>
        </div>
      ))}
      {status.addresses.length > 0 ? (
        <div className="thread-addresses">
          <dt>Addresses</dt>
          <dd>{status.addresses.join(', ')}</dd>
        </div>
      ) : null}
    </dl>
  );
}

function optionalValue(value: string) {
  const trimmed = value.trim();
  return trimmed || undefined;
}
