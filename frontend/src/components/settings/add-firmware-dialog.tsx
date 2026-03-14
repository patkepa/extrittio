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
import { useDeviceTypes } from '../../hooks/use-device-types';
import {
  useCreateFirmwareUpdate,
  useNextVersion,
  useUploadFirmwareUpdate,
} from '../../hooks/use-firmware-updates';

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
  const [selectedDeviceTypeId, setSelectedDeviceTypeId] = useState<number | null>(null);
  const [version, setVersion] = useState('');
  const [url, setUrl] = useState('');
  const [sha256, setSha256] = useState('');
  const [description, setDescription] = useState('');
  const [uploadMode, setUploadMode] = useState<'file' | 'url'>('file');
  const [selectedFile, setSelectedFile] = useState<File | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const { data: deviceTypes = [] } = useDeviceTypes();
  const createMutation = useCreateFirmwareUpdate();
  const uploadMutation = useUploadFirmwareUpdate();
  const { data: nextVersion } = useNextVersion(selectedDeviceTypeId);

  const isSubmitting = createMutation.isPending || uploadMutation.isPending;
  const isError = createMutation.isError || uploadMutation.isError;

  const canSubmit =
    !!selectedDeviceTypeId && (uploadMode === 'file' ? !!selectedFile : !!url.trim());

  const handleAdd = () => {
    if (!selectedDeviceTypeId) return;

    if (uploadMode === 'file') {
      if (!selectedFile) return;
      uploadMutation.mutate(
        {
          device_type_id: selectedDeviceTypeId,
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
      if (!url.trim()) return;
      createMutation.mutate(
        {
          device_type_id: selectedDeviceTypeId,
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
    setSelectedDeviceTypeId(null);
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

  return (
    <Dialog icon="upload" title="Register Firmware Update" isOpen={isOpen} onClose={handleClose}>
      <DialogBody>
        <FormGroup label="Device Type" labelInfo="(required)">
          <HTMLSelect
            value={selectedDeviceTypeId ?? ''}
            onChange={(e) => {
              const val = e.target.value;
              setSelectedDeviceTypeId(val ? Number(val) : null);
              setVersion('');
            }}
            fill
          >
            <option value="">Select a device type...</option>
            {deviceTypes.map((dt) => (
              <option key={dt.id} value={dt.id}>
                {dt.name}
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
              helperText="Optional hash for binary verification on device"
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
            Failed to create firmware update. Version may already exist for this device type.
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
