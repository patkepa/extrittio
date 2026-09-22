import { useRef, useState } from 'react';
import { showSuccessToast, showErrorToast } from '../../utils/toaster';
import {
  Button,
  Callout,
  Dialog,
  DialogBody,
  DialogFooter,
  FormGroup,
  HTMLSelect,
  InputGroup,
  SegmentedControl,
  TextArea,
} from '@blueprintjs/core';
import {
  useCreateFirmwareUpdate,
  useNextBlueprintVersion,
  useUploadFirmwareUpdate,
} from '../../hooks/use-firmware-updates';
import {
  useDeviceBlueprints,
  useLatestDeviceBlueprintRevision,
} from '../../hooks/use-device-blueprints';
import { useConfirmShortcut } from '@patkepa/kantzen-ui/interactions';

function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

interface AddFirmwareDialogProps {
  isOpen: boolean;
  onClose: () => void;
}

export const AddFirmwareDialog = ({ isOpen, onClose }: AddFirmwareDialogProps) => {
  const [selectedBlueprintId, setSelectedBlueprintId] = useState('');
  const [version, setVersion] = useState('');
  const [url, setUrl] = useState('');
  const [sha256, setSha256] = useState('');
  const [description, setDescription] = useState('');
  const [uploadMode, setUploadMode] = useState<'file' | 'url'>('file');
  const [selectedFile, setSelectedFile] = useState<File | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const { data: blueprints = [] } = useDeviceBlueprints();
  const publishedBlueprints = blueprints.filter((blueprint) => blueprint.latest_revision != null);
  const effectiveBlueprintId = selectedBlueprintId || publishedBlueprints[0]?.id || '';
  const revisionQuery = useLatestDeviceBlueprintRevision(effectiveBlueprintId);
  const firmwareDefinition = (
    revisionQuery.data?.document as
      | {
          spec?: { firmware?: { strategy?: string } };
        }
      | undefined
  )?.spec?.firmware;
  const createMutation = useCreateFirmwareUpdate();
  const uploadMutation = useUploadFirmwareUpdate();
  const { data: nextVersion } = useNextBlueprintVersion(revisionQuery.data?.id ?? null);

  const isSubmitting = createMutation.isPending || uploadMutation.isPending;
  const isError = createMutation.isError || uploadMutation.isError;

  const canSubmit =
    !!revisionQuery.data &&
    !!firmwareDefinition &&
    (uploadMode === 'file'
      ? !!selectedFile
      : url.trim().startsWith('https://') && /^[a-fA-F0-9]{64}$/.test(sha256.trim())) &&
    !isSubmitting;

  const handleAdd = () => {
    if (!revisionQuery.data || !firmwareDefinition) return;

    if (uploadMode === 'file') {
      if (!selectedFile) return;
      uploadMutation.mutate(
        {
          blueprint_revision_id: revisionQuery.data.id,
          version: version.trim() || undefined,
          description: description.trim() || undefined,
          file: selectedFile,
        },
        {
          onSuccess: () => {
            onClose();
            void showSuccessToast('Firmware uploaded');
          },
          onError: () => void showErrorToast('Failed to upload firmware'),
        },
      );
    } else {
      if (!url.trim() || !sha256.trim()) return;
      createMutation.mutate(
        {
          blueprint_revision_id: revisionQuery.data.id,
          version: version.trim() || undefined,
          url: url.trim(),
          sha256: sha256.trim() || undefined,
          description: description.trim() || undefined,
        },
        {
          onSuccess: () => {
            onClose();
            void showSuccessToast('Firmware registered');
          },
          onError: () => void showErrorToast('Failed to register firmware'),
        },
      );
    }
  };

  const handleClose = () => {
    setSelectedBlueprintId('');
    setVersion('');
    setUrl('');
    setSha256('');
    setDescription('');
    setSelectedFile(null);
    setUploadMode('file');
    createMutation.reset();
    uploadMutation.reset();
    onClose();
  };

  useConfirmShortcut({
    isOpen,
    canConfirm: canSubmit,
    onConfirm: handleAdd,
  });

  return (
    <Dialog icon="upload" title="Register Firmware Update" isOpen={isOpen} onClose={handleClose}>
      <DialogBody>
        <FormGroup
          label="Device Blueprint"
          labelInfo="(required)"
          helperText={
            revisionQuery.data && !firmwareDefinition
              ? 'This blueprint does not declare firmware update behavior.'
              : firmwareDefinition?.strategy
                ? `Update strategy: ${firmwareDefinition.strategy}`
                : undefined
          }
        >
          <HTMLSelect
            value={effectiveBlueprintId}
            onChange={(e) => {
              setSelectedBlueprintId(e.target.value);
              setVersion('');
            }}
            fill
          >
            {publishedBlueprints.length === 0 && (
              <option value="">Publish a blueprint before adding firmware</option>
            )}
            {publishedBlueprints.map((blueprint) => (
              <option key={blueprint.id} value={blueprint.id}>
                {blueprint.name} · revision {blueprint.latest_revision}
              </option>
            ))}
          </HTMLSelect>
        </FormGroup>
        <FormGroup
          label="Version"
          helperText={nextVersion ? `Next suggested: ${nextVersion.next_version}` : undefined}
        >
          <InputGroup
            placeholder={nextVersion?.next_version ?? 'e.g. 1.0.0'}
            value={version}
            onChange={(e) => setVersion(e.target.value)}
            className="mono-data"
          />
        </FormGroup>

        <FormGroup label="Source" labelInfo="(required)">
          <SegmentedControl
            options={[
              { label: 'Upload File', value: 'file' },
              { label: 'External URL', value: 'url' },
            ]}
            value={uploadMode}
            onValueChange={(val) => setUploadMode(val as 'file' | 'url')}
            small
            fill
          />
        </FormGroup>

        {uploadMode === 'file' ? (
          <FormGroup
            label="Firmware File"
            labelInfo="(required)"
            helperText="Binary will be stored in the database. SHA-256 is computed automatically."
          >
            <input
              ref={fileInputRef}
              type="file"
              style={{ display: 'none' }}
              onChange={(e) => setSelectedFile(e.target.files?.[0] ?? null)}
            />
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              <Button icon="document-open" onClick={() => fileInputRef.current?.click()}>
                Choose File
              </Button>
              {selectedFile ? (
                <span style={{ fontSize: 13 }}>
                  <span className="mono-data">{selectedFile.name}</span>
                  <span style={{ opacity: 0.6, marginLeft: 8 }}>
                    ({formatFileSize(selectedFile.size)})
                  </span>
                </span>
              ) : (
                <span style={{ opacity: 0.5, fontSize: 13 }}>No file selected</span>
              )}
            </div>
          </FormGroup>
        ) : (
          <>
            <FormGroup label="Firmware URL" labelInfo="(required)">
              <InputGroup
                placeholder="https://releases.example.com/firmware/v1.0.0.bin"
                value={url}
                onChange={(e) => setUrl(e.target.value)}
                className="mono-data"
              />
            </FormGroup>
            <FormGroup
              label="SHA-256 Hash"
              labelInfo="(required)"
              helperText="Required for device-side binary verification"
            >
              <InputGroup
                placeholder="e.g. a1b2c3d4..."
                value={sha256}
                onChange={(e) => setSha256(e.target.value)}
                className="mono-data"
              />
            </FormGroup>
          </>
        )}

        <FormGroup label="Description">
          <TextArea
            placeholder="Release notes or changelog..."
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            fill
            rows={3}
          />
        </FormGroup>
        {isError && (
          <Callout intent="danger" icon="error">
            Failed to create firmware update. This version may already exist for the selected blueprint revision.
          </Callout>
        )}
      </DialogBody>
      <DialogFooter
        actions={
          <>
            <Button onClick={handleClose}>Cancel</Button>
            <Button
              intent="primary"
              icon="upload"
              onClick={handleAdd}
              loading={isSubmitting}
              disabled={!canSubmit}
            >
              {uploadMode === 'file' ? 'Upload' : 'Register'}
            </Button>
          </>
        }
      />
    </Dialog>
  );
};
