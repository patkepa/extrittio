import { useState } from 'react';
import {
  Alert,
  Button,
  Callout,
  Card,
  Elevation,
  FormGroup,
  H3,
  HTMLSelect,
  InputGroup,
  Spinner,
  Tag,
} from '@blueprintjs/core';
import { QRCodeSVG } from 'qrcode.react';
import {
  useCreateThreadNetwork,
  useImportThreadDataset,
  useRevealThreadDataset,
  useThreadStatus,
} from '../../hooks/use-thread';
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

  const createMutation = useCreateThreadNetwork();
  const importMutation = useImportThreadDataset();
  const statusQuery = useThreadStatus();
  const datasetMutation = useRevealThreadDataset();

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
            datasetMutation.reset();
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
            datasetMutation.reset();
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

  return (
    <div className="settings-page">
      <div className="page-header">
        <div>
          <H3>OpenThread Settings</H3>
          <p className="page-description">
            Configure the operational dataset used by the local OpenThread border router.
          </p>
        </div>
      </div>

      <div className="settings-content">
        <Card elevation={Elevation.ONE} className="settings-card thread-active-card">
          <div className="thread-card-heading">
            <span className="section-label">Current Thread Network</span>
            {statusQuery.data ? (
              <Tag intent={statusQuery.data.connected ? 'success' : 'warning'} minimal round>
                {statusQuery.data.connected ? 'Connected' : 'Not connected'}
              </Tag>
            ) : null}
          </div>

          {statusQuery.isPending ? (
            <div className="thread-status-loading">
              <Spinner size={20} />
              Loading the active network…
            </div>
          ) : statusQuery.isError ? (
            <Callout intent="danger" icon="error" title="Thread status is unavailable">
              Refresh the page to try again.
            </Callout>
          ) : !statusQuery.data?.available ? (
            <Callout intent="warning" icon="offline" title="OpenThread is unavailable">
              Connect a compatible RCP to inspect or configure a Thread network.
            </Callout>
          ) : (
            <>
              <dl className="thread-network-details">
                <ThreadDetail label="Network name" value={statusQuery.data.network_name} />
                <ThreadDetail label="Role" value={statusQuery.data.role} />
                <ThreadDetail
                  label="Channel"
                  value={statusQuery.data.channel?.toString() ?? null}
                />
                <ThreadDetail label="PAN ID" value={statusQuery.data.pan_id} />
                <ThreadDetail label="Extended PAN ID" value={statusQuery.data.extended_pan_id} />
                <ThreadDetail
                  label="Mesh-local prefix"
                  value={statusQuery.data.mesh_local_prefix}
                />
              </dl>

              {!statusQuery.data.connected ? (
                <Callout intent="warning" icon="warning-sign">
                  {statusQuery.data.error ??
                    'The border router is not attached to a Thread network.'}
                </Callout>
              ) : datasetMutation.data ? (
                <div className="thread-credentials">
                  <Callout intent="warning" icon="key" title="Keep these credentials private">
                    Anyone with this dataset can provision a device onto this Thread network.
                  </Callout>

                  <div className="thread-credentials-layout">
                    <div className="thread-secret-details">
                      <ThreadSecret
                        label="Network key"
                        value={datasetMutation.data.network_key}
                        onCopy={() =>
                          void copyCredential(datasetMutation.data.network_key, 'Network key')
                        }
                      />
                      <ThreadSecret
                        label="Commissioner PSKc"
                        value={datasetMutation.data.pskc}
                        onCopy={() => void copyCredential(datasetMutation.data.pskc, 'PSKc')}
                      />
                      <ThreadSecret
                        label="Active Operational Dataset (hex TLVs)"
                        value={datasetMutation.data.active_dataset_tlvs}
                        multiline
                        onCopy={() =>
                          void copyCredential(
                            datasetMutation.data.active_dataset_tlvs,
                            'Active dataset',
                          )
                        }
                      />
                    </div>

                    <div className="thread-dataset-qr">
                      <div className="thread-qr-code" aria-label="Active Thread dataset QR code">
                        <QRCodeSVG
                          value={datasetMutation.data.active_dataset_tlvs}
                          size={208}
                          bgColor="#ffffff"
                          fgColor="#000000"
                          level="M"
                          marginSize={2}
                          title="Active Thread operational dataset"
                        />
                      </div>
                      <strong>Scan to provision</strong>
                      <span>
                        For Thread provisioning apps and devices that accept Active Operational
                        Dataset TLVs.
                      </span>
                    </div>
                  </div>

                  <Button icon="eye-off" onClick={() => datasetMutation.reset()}>
                    Hide credentials
                  </Button>
                </div>
              ) : (
                <div className="thread-reveal">
                  <p className="thread-help">
                    Secure credentials and the provisioning QR code are loaded only when you ask to
                    see them.
                  </p>
                  {datasetMutation.isError ? (
                    <Callout intent="danger" icon="error">
                      Unable to read the Active Operational Dataset from OpenThread.
                    </Callout>
                  ) : null}
                  <Button
                    icon="eye-open"
                    loading={datasetMutation.isPending}
                    onClick={() => datasetMutation.mutate()}
                  >
                    Show credentials &amp; QR
                  </Button>
                </div>
              )}
            </>
          )}
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

function optionalValue(value: string) {
  const trimmed = value.trim();
  return trimmed || undefined;
}

function ThreadDetail({ label, value }: { label: string; value: string | null }) {
  return (
    <div>
      <dt>{label}</dt>
      <dd>{value ?? 'Not reported'}</dd>
    </div>
  );
}

function ThreadSecret({
  label,
  value,
  multiline = false,
  onCopy,
}: {
  label: string;
  value: string | null;
  multiline?: boolean;
  onCopy: () => void;
}) {
  return (
    <div className="thread-secret">
      <span>{label}</span>
      <div className="thread-secret-value">
        <code className={multiline ? 'thread-secret-code thread-secret-code--multiline' : ''}>
          {value ?? 'Not present in the active dataset'}
        </code>
        <Button
          icon="duplicate"
          minimal
          aria-label={`Copy ${label}`}
          disabled={!value}
          onClick={onCopy}
        />
      </div>
    </div>
  );
}

async function copyCredential(value: string | null, label: string) {
  if (!value) return;
  try {
    await navigator.clipboard.writeText(value);
    void showSuccessToast(`${label} copied to clipboard`);
  } catch {
    void showErrorToast(`Unable to copy ${label.toLowerCase()}`);
  }
}
