import {
  Button,
  ButtonGroup,
  Callout,
  Dialog,
  DialogBody,
  DialogFooter,
  Menu,
  MenuItem,
  Popover,
} from '@blueprintjs/core';
import type { DeviceCertificateResponse } from '../../types/api';
import { downloadPem } from '../../utils/download-pem';

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

  const safeName = deviceName.replace(/[^a-zA-Z0-9_-]/g, '-');

  const downloadAll = () => {
    downloadPem(certBundle.certificate_pem, `${safeName}.pem`);
    setTimeout(() => downloadPem(certBundle.private_key_pem, `${safeName}-key.pem`), 100);
    setTimeout(() => downloadPem(certBundle.ca_pem, 'ca.pem'), 200);
  };

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
        <ButtonGroup style={{ display: 'flex', width: '100%' }}>
          <Button icon="download" intent="primary" fill onClick={downloadAll}>
            Download All
          </Button>
          <Popover
            interactionKind="hover"
            placement="bottom-end"
            content={
              <Menu>
                <MenuItem
                  icon="document"
                  text="Device Certificate"
                  onClick={() => downloadPem(certBundle.certificate_pem, `${safeName}.pem`)}
                />
                <MenuItem
                  icon="key"
                  text="Private Key"
                  onClick={() => downloadPem(certBundle.private_key_pem, `${safeName}-key.pem`)}
                />
                <MenuItem
                  icon="shield"
                  text="CA Certificate"
                  onClick={() => downloadPem(certBundle.ca_pem, 'ca.pem')}
                />
              </Menu>
            }
          >
            <Button intent="primary" icon="caret-down" />
          </Popover>
        </ButtonGroup>
        <div style={{ marginTop: 12 }}>
          <div style={{ fontSize: 11, color: 'hsl(var(--muted))', marginBottom: 4 }}>
            Fingerprint
          </div>
          <code
            style={{
              display: 'block',
              fontSize: 11,
              padding: '6px 8px',
              background: 'rgba(0,0,0,0.25)',
              borderRadius: 3,
              wordBreak: 'break-all',
              lineHeight: 1.6,
              color: 'hsl(var(--muted))',
            }}
          >
            {certBundle.fingerprint}
          </code>
        </div>
      </DialogBody>
      <DialogFooter actions={<Button onClick={onClose}>Done</Button>} />
    </Dialog>
  );
}
