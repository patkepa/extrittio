import { Button, Callout, Dialog, DialogBody, DialogFooter, Icon } from "@blueprintjs/core";
import type { DeviceCertificateResponse } from "../../types/api";
import { downloadPem } from "../../utils/download-pem";

interface CertificateDownloadDialogProps {
  isOpen: boolean;
  onClose: () => void;
  certBundle: DeviceCertificateResponse | null;
  deviceName: string;
}

export function CertificateDownloadDialog({
  isOpen,
  onClose,
  certBundle,
  deviceName,
}: CertificateDownloadDialogProps) {
  if (!certBundle) return null;

  const safeName = deviceName.replace(/[^a-zA-Z0-9_-]/g, "-");

  return (
    <Dialog
      icon="lock"
      title="Certificate Bundle"
      isOpen={isOpen}
      onClose={onClose}
      canOutsideClickClose={false}
    >
      <DialogBody>
        <Callout intent="warning" icon="warning-sign" style={{ marginBottom: 16 }}>
          Save these files now. The private key will not be available again.
        </Callout>
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          <Button
            icon="download"
            onClick={() => downloadPem(certBundle.certificate_pem, `${safeName}.pem`)}
          >
            Download Device Certificate
          </Button>
          <Button
            icon="download"
            intent="primary"
            onClick={() => downloadPem(certBundle.private_key_pem, `${safeName}-key.pem`)}
          >
            Download Private Key
          </Button>
          <Button
            icon="download"
            onClick={() => downloadPem(certBundle.ca_pem, "ca.pem")}
          >
            Download CA Certificate
          </Button>
        </div>
        <div style={{ marginTop: 16, fontSize: 12, color: "hsl(var(--muted))" }}>
          <Icon icon="info-sign" size={12} style={{ marginRight: 4 }} />
          Fingerprint: <code>{certBundle.fingerprint}</code>
        </div>
      </DialogBody>
      <DialogFooter
        actions={<Button onClick={onClose}>Done</Button>}
      />
    </Dialog>
  );
}
