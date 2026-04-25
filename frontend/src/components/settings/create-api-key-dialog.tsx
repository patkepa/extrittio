import { useState } from 'react';
import {
  Button,
  Classes,
  Dialog,
  DialogBody,
  DialogFooter,
  FormGroup,
  InputGroup,
  HTMLSelect,
  Callout,
  Code,
} from '@blueprintjs/core';
import { useCreateApiKey } from '../../hooks/use-api-keys';
import { useConfirmShortcut } from '../../hooks/use-confirm-shortcut';
import { useDeviceTypes } from '../../hooks/use-device-types';

interface Props {
  isOpen: boolean;
  onClose: () => void;
}

export const CreateApiKeyDialog = ({ isOpen, onClose }: Props) => {
  const [name, setName] = useState('');
  const [deviceTypeId, setDeviceTypeId] = useState<number | undefined>();
  const [createdKey, setCreatedKey] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const createMutation = useCreateApiKey();
  const { data: deviceTypes } = useDeviceTypes();
  const canCreate = !createdKey && !!name.trim() && !createMutation.isPending;

  const handleCreate = async () => {
    const result = await createMutation.mutateAsync({
      name,
      device_type_id: deviceTypeId,
    });
    setCreatedKey(result.key);
  };

  const handleCopy = () => {
    if (createdKey) {
      navigator.clipboard.writeText(createdKey);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  const handleClose = () => {
    setName('');
    setDeviceTypeId(undefined);
    setCreatedKey(null);
    setCopied(false);
    onClose();
  };

  useConfirmShortcut({
    isOpen,
    canConfirm: canCreate,
    onConfirm: handleCreate,
  });

  return (
    <Dialog isOpen={isOpen} onClose={handleClose} title="Create API Key">
      <DialogBody>
        {createdKey ? (
          <Callout intent="warning" title="Save this key now">
            <p>This key will only be shown once. Copy it before closing this dialog.</p>
            <div style={{ display: 'flex', gap: 8, alignItems: 'center', marginTop: 8 }}>
              <Code style={{ flex: 1, wordBreak: 'break-all' }}>{createdKey}</Code>
              <Button
                icon={copied ? 'tick' : 'clipboard'}
                intent={copied ? 'success' : 'none'}
                onClick={handleCopy}
              >
                {copied ? 'Copied' : 'Copy'}
              </Button>
            </div>
          </Callout>
        ) : (
          <>
            <FormGroup label="Name" labelFor="key-name">
              <InputGroup
                id="key-name"
                placeholder="e.g. ESP32 CI Pipeline"
                value={name}
                onChange={(e) => setName(e.target.value)}
              />
            </FormGroup>
            <FormGroup
              label="Scope (optional)"
              helperText="Restrict this key to a specific device type"
            >
              <HTMLSelect
                value={deviceTypeId ?? ''}
                onChange={(e) =>
                  setDeviceTypeId(e.target.value ? Number(e.target.value) : undefined)
                }
              >
                <option value="">All device types</option>
                {deviceTypes?.map((dt) => (
                  <option key={dt.id} value={dt.id}>
                    {dt.name}
                  </option>
                ))}
              </HTMLSelect>
            </FormGroup>
          </>
        )}
      </DialogBody>
      <DialogFooter
        actions={
          createdKey ? (
            <Button onClick={handleClose}>Done</Button>
          ) : (
            <>
              <Button onClick={handleClose} className={Classes.DIALOG_CLOSE_BUTTON}>
                Cancel
              </Button>
              <Button
                intent="primary"
                onClick={handleCreate}
                loading={createMutation.isPending}
                disabled={!canCreate}
              >
                Create Key
              </Button>
            </>
          )
        }
      />
    </Dialog>
  );
};
