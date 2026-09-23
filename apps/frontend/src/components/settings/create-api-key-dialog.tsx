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
import { useConfirmShortcut } from '@patkepa/kantzen-ui/interactions';
import { useDeviceBlueprints } from '../../hooks/use-device-blueprints';

interface Props {
  isOpen: boolean;
  onClose: () => void;
}

export const CreateApiKeyDialog = ({ isOpen, onClose }: Props) => {
  const [name, setName] = useState('');
  const [blueprintId, setBlueprintId] = useState<string | undefined>();
  const [createdKey, setCreatedKey] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const createMutation = useCreateApiKey();
  const {
    data: blueprints,
    isLoading: loadingBlueprints,
    error: blueprintError,
  } = useDeviceBlueprints();
  const canCreate =
    !createdKey &&
    !!name.trim() &&
    !createMutation.isPending &&
    !loadingBlueprints &&
    !blueprintError;

  const handleCreate = async () => {
    const result = await createMutation.mutateAsync({
      name,
      blueprint_id: blueprintId,
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
    setBlueprintId(undefined);
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
            {blueprintError ? <Callout intent="danger">Failed to load blueprints.</Callout> : null}
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
              helperText="Restrict this key to a specific blueprint"
            >
              <HTMLSelect
                value={blueprintId ?? ''}
                onChange={(e) => setBlueprintId(e.target.value ? e.target.value : undefined)}
              >
                <option value="">All blueprints (tenant-wide)</option>
                {blueprints?.map((dt) => (
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
