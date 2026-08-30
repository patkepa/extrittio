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
import type { DeviceCertificateResponse, DeviceContract } from '../../types/api';
import { downloadPem } from '../../utils/download-pem';

interface CertificateDownloadDialogProps {
  isOpen: boolean;
  onClose: () => void;
  certBundle: DeviceCertificateResponse | null;
  contract: DeviceContract | null;
  deviceName: string;
}

export function CertificateDownloadDialog({
  isOpen,
  onClose,
  certBundle,
  contract,
  deviceName,
}: CertificateDownloadDialogProps) {
  if (!certBundle && !contract) return null;

  const safeName = deviceName.replace(/[^a-zA-Z0-9_-]/g, '-');

  const downloadContract = () => {
    if (!contract) return;
    const blob = new Blob([JSON.stringify(contract, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = `${safeName}-contract.json`;
    anchor.click();
    URL.revokeObjectURL(url);
  };

  const downloadAll = () => {
    downloadContract();
    if (certBundle) {
      setTimeout(() => downloadPem(certBundle.certificate_pem, `${safeName}.pem`), 100);
      setTimeout(() => downloadPem(certBundle.private_key_pem, `${safeName}-key.pem`), 200);
      setTimeout(() => downloadPem(certBundle.ca_pem, 'ca.pem'), 300);
    }
  };

  return (
    <Dialog
      icon="lock"
      title="Device Provisioning Bundle"
      isOpen={isOpen}
      onClose={onClose}
      canOutsideClickClose={false}
    >
      <DialogBody>
        <Callout
          intent={certBundle ? 'warning' : 'primary'}
          icon="warning-sign"
          style={{ marginBottom: 16 }}
        >
          {certBundle
            ? 'Save these files now. The private key will not be available again.'
            : 'Save the contract with the device. It contains its complete runtime configuration.'}
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
                {contract && (
                  <MenuItem icon="code" text="Device Contract" onClick={downloadContract} />
                )}
                {certBundle && (
                  <>
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
                  </>
                )}
              </Menu>
            }
          >
            <Button intent="primary" icon="caret-down" />
          </Popover>
        </ButtonGroup>
        {certBundle && (
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
        )}
      </DialogBody>
      <DialogFooter actions={<Button onClick={onClose}>Done</Button>} />
    </Dialog>
  );
}
