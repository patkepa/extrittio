import { useState } from "react";
import {
  Alert,
  Button,
  Callout,
  Card,
  Elevation,
  H3,
  HTMLTable,
  Icon,
  Spinner,
  Tooltip,
} from "@blueprintjs/core";
import { useDevices } from "../../hooks/use-devices";
import {
  useCaCertificate,
  useDeviceCertificateStatuses,
  useRegenerateDeviceCertificate,
} from "../../hooks/use-certificates";
import { CertificateDownloadDialog } from "../../components/certificates/certificate-download-dialog";
import { downloadPem } from "../../utils/download-pem";
import { showSuccessToast, showErrorToast } from "../../utils/toaster";
import type { DeviceCertificateResponse } from "../../types/api";
import "./settings.css";
import "./certificates.css";

export function CertificatesSettings() {
  const [regenerateTarget, setRegenerateTarget] = useState<{
    id: string;
    name: string;
  } | null>(null);
  const [downloadBundle, setDownloadBundle] = useState<{
    bundle: DeviceCertificateResponse;
    name: string;
  } | null>(null);

  const caQuery = useCaCertificate();
  const devicesQuery = useDevices();
  const devices = devicesQuery.data?.data ?? [];
  const deviceIds = devices.map((d) => d.id);
  const statusQueries = useDeviceCertificateStatuses(deviceIds);
  const regenerateMutation = useRegenerateDeviceCertificate();

  const handleRegenerate = () => {
    if (!regenerateTarget) return;
    const target = regenerateTarget;
    setRegenerateTarget(null);
    regenerateMutation.mutate(target.id, {
      onSuccess: (data) => {
        void showSuccessToast("Certificate regenerated");
        setDownloadBundle({ bundle: data, name: target.name });
      },
      onError: () => {
        void showErrorToast("Failed to regenerate certificate");
      },
    });
  };

  const copyToClipboard = (text: string) => {
    void navigator.clipboard.writeText(text);
    void showSuccessToast("Copied to clipboard");
  };

  const targetIdx = regenerateTarget
    ? devices.findIndex((d) => d.id === regenerateTarget.id)
    : -1;
  const targetHasCert = targetIdx >= 0 && !!statusQueries[targetIdx]?.data;

  return (
    <div className="settings-page">
      <div className="page-header">
        <div>
          <H3>Certificates</H3>
          <p className="page-description">
            Manage CA and device TLS certificates
          </p>
        </div>
      </div>

      {/* CA Certificate */}
      <span className="section-label">Certificate Authority</span>
      {caQuery.isLoading ? (
        <Spinner size={24} />
      ) : caQuery.isError ? (
        <Callout intent="warning" icon="info-sign" style={{ marginBottom: 24 }}>
          No CA certificate found. TLS may not be enabled on the backend.
        </Callout>
      ) : caQuery.data ? (
        <Card elevation={Elevation.ONE} className="ca-card">
          <div className="ca-card-content">
            <div className="ca-card-details">
              <Tooltip content="Click to copy">
                <div
                  className="ca-fingerprint"
                  style={{ cursor: "pointer" }}
                  onClick={() => copyToClipboard(caQuery.data!.fingerprint)}
                >
                  {caQuery.data.fingerprint}
                </div>
              </Tooltip>
              <div style={{ fontSize: 12, color: "hsl(var(--muted))" }}>
                Created: {new Date(caQuery.data.created_at).toLocaleDateString()}
              </div>
            </div>
            <Button
              icon="download"
              onClick={() => downloadPem(caQuery.data!.certificate_pem, "ca.pem")}
            >
              Download CA PEM
            </Button>
          </div>
        </Card>
      ) : null}

      {/* Device Certificates */}
      <span className="section-label" style={{ marginTop: 8, display: "block" }}>
        Device Certificates
      </span>
      {devicesQuery.isLoading ? (
        <Spinner size={24} />
      ) : devicesQuery.isError ? (
        <Callout intent="danger" icon="error">
          Failed to load devices
        </Callout>
      ) : devices.length === 0 ? (
        <div className="settings-empty">
          <Icon icon="lock" size={32} />
          <p>No devices registered</p>
        </div>
      ) : (
        <Card elevation={Elevation.ONE} className="settings-table-card">
          <HTMLTable className="settings-table" interactive>
            <thead>
              <tr>
                <th>Device</th>
                <th>Fingerprint</th>
                <th>Expires</th>
                <th>Created</th>
                <th className="actions-column">Actions</th>
              </tr>
            </thead>
            <tbody>
              {devices.map((device, idx) => {
                const statusQuery = statusQueries[idx];
                const status = statusQuery?.data;
                const isLoading = statusQuery?.isLoading;

                return (
                  <tr key={device.id}>
                    <td>{device.name}</td>
                    <td>
                      {isLoading ? (
                        <Spinner size={14} />
                      ) : status ? (
                        <Tooltip content={status.fingerprint}>
                          <span className="cert-fingerprint">
                            {status.fingerprint}
                          </span>
                        </Tooltip>
                      ) : (
                        <span className="cert-no-cert">No certificate</span>
                      )}
                    </td>
                    <td>
                      {status
                        ? new Date(status.expires_at).toLocaleDateString()
                        : "—"}
                    </td>
                    <td>
                      {status
                        ? new Date(status.created_at).toLocaleDateString()
                        : "—"}
                    </td>
                    <td className="actions-column">
                      <Button
                        small
                        minimal
                        intent="primary"
                        rightIcon="refresh"
                        loading={
                          regenerateMutation.isPending &&
                          regenerateMutation.variables === device.id
                        }
                        onClick={() =>
                          setRegenerateTarget({
                            id: device.id,
                            name: device.name,
                          })
                        }
                      >
                        {status ? "Regenerate" : "Generate"}
                      </Button>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </HTMLTable>
        </Card>
      )}

      {/* Regenerate confirmation */}
      <Alert
        isOpen={regenerateTarget !== null}
        onConfirm={handleRegenerate}
        onCancel={() => setRegenerateTarget(null)}
        cancelButtonText="Cancel"
        confirmButtonText={targetHasCert ? "Regenerate" : "Generate"}
        intent="warning"
        icon="refresh"
      >
        <p>
          {targetHasCert
            ? "This will revoke the current certificate and issue a new one. The device will need to be reconfigured with the new certificate."
            : `Generate a certificate for ${regenerateTarget?.name}?`}
        </p>
      </Alert>

      {/* Download dialog after regeneration */}
      <CertificateDownloadDialog
        isOpen={downloadBundle !== null}
        onClose={() => setDownloadBundle(null)}
        certBundle={downloadBundle?.bundle ?? null}
        deviceName={downloadBundle?.name ?? ""}
      />
    </div>
  );
}
