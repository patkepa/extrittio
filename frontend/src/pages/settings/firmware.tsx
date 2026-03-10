import { useRef, useState } from 'react';
import {
  Card,
  Elevation,
  H3,
  HTMLTable,
  Button,
  Dialog,
  DialogBody,
  DialogFooter,
  FormGroup,
  InputGroup,
  Callout,
  Tag,
  Spinner,
  HTMLSelect,
  TextArea,
  SegmentedControl,
} from '@blueprintjs/core';
import { useDeviceTypes } from '../../hooks/use-device-types';
import {
  useFirmwareUpdates,
  useCreateFirmwareUpdate,
  useUploadFirmwareUpdate,
  useDeleteFirmwareUpdate,
  useNextVersion,
} from '../../hooks/use-firmware-updates';
import './settings.css';

function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export const FirmwareSettings = () => {
  const [isAddDialogOpen, setIsAddDialogOpen] = useState(false);
  const [selectedDeviceTypeId, setSelectedDeviceTypeId] = useState<number | null>(null);
  const [version, setVersion] = useState('');
  const [url, setUrl] = useState('');
  const [sha256, setSha256] = useState('');
  const [description, setDescription] = useState('');
  const [filterDeviceTypeId, setFilterDeviceTypeId] = useState<number | undefined>(undefined);
  const [uploadMode, setUploadMode] = useState<'file' | 'url'>('file');
  const [selectedFile, setSelectedFile] = useState<File | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const { data: deviceTypes = [] } = useDeviceTypes();
  const { data: firmwareUpdates = [], isLoading, error } = useFirmwareUpdates(
    filterDeviceTypeId ? { device_type_id: filterDeviceTypeId } : undefined
  );
  const createMutation = useCreateFirmwareUpdate();
  const uploadMutation = useUploadFirmwareUpdate();
  const deleteMutation = useDeleteFirmwareUpdate();
  const { data: nextVersion } = useNextVersion(selectedDeviceTypeId);

  const openAddDialog = () => {
    setSelectedDeviceTypeId(deviceTypes[0]?.id ?? null);
    setVersion('');
    setUrl('');
    setSha256('');
    setDescription('');
    setSelectedFile(null);
    setUploadMode('file');
    setIsAddDialogOpen(true);
  };

  const isSubmitting = createMutation.isPending || uploadMutation.isPending;
  const isError = createMutation.isError || uploadMutation.isError;

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
        { onSuccess: () => setIsAddDialogOpen(false) }
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
        { onSuccess: () => setIsAddDialogOpen(false) }
      );
    }
  };

  const canSubmit =
    !!selectedDeviceTypeId &&
    (uploadMode === 'file' ? !!selectedFile : !!url.trim());

  if (error) {
    return (
      <div className="settings-page">
        <Callout intent="danger" icon="error">
          Failed to load firmware updates. Is the backend running?
        </Callout>
      </div>
    );
  }

  if (isLoading) {
    return (
      <div className="settings-page">
        <Spinner />
      </div>
    );
  }

  return (
    <div className="settings-page">
      <div className="page-header">
        <div>
          <H3>Firmware Updates</H3>
          <p className="page-description">
            {firmwareUpdates.length} firmware release{firmwareUpdates.length !== 1 ? 's' : ''} registered
          </p>
        </div>
        <Button intent="primary" icon="add" onClick={openAddDialog}>
          Add Firmware
        </Button>
      </div>

      <div style={{ marginBottom: 16 }}>
        <HTMLSelect
          value={filterDeviceTypeId ?? ''}
          onChange={(e) =>
            setFilterDeviceTypeId(
              e.target.value ? Number(e.target.value) : undefined
            )
          }
          style={{ minWidth: 200 }}
        >
          <option value="">All device types</option>
          {deviceTypes.map((dt) => (
            <option key={dt.id} value={dt.id}>
              {dt.name}
            </option>
          ))}
        </HTMLSelect>
      </div>

      <Card elevation={Elevation.TWO} className="settings-table-card">
        {firmwareUpdates.length === 0 ? (
          <div className="settings-empty">
            <p>No firmware updates found. Add one to get started.</p>
          </div>
        ) : (
          <HTMLTable interactive className="settings-table">
            <thead>
              <tr>
                <th>Version</th>
                <th>Device Type</th>
                <th>Source</th>
                <th>Description</th>
                <th className="actions-column">Actions</th>
              </tr>
            </thead>
            <tbody>
              {firmwareUpdates.map((fw) => (
                <tr key={fw.id}>
                  <td>
                    <Tag minimal intent="primary" className="mono-data">
                      v{fw.version}
                    </Tag>
                  </td>
                  <td>{fw.device_type_name}</td>
                  <td>
                    {fw.has_blob ? (
                      <span style={{ fontSize: 12 }}>
                        <Tag minimal intent="success" icon="document" style={{ marginRight: 6 }}>
                          {fw.filename}
                        </Tag>
                        <span style={{ opacity: 0.6 }}>
                          {fw.file_size != null ? formatFileSize(fw.file_size) : ''}
                        </span>
                      </span>
                    ) : (
                      <span
                        className="mono-data"
                        style={{ fontSize: 12, opacity: 0.8, maxWidth: 300, display: 'inline-block', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}
                        title={fw.url}
                      >
                        {fw.url}
                      </span>
                    )}
                  </td>
                  <td style={{ maxWidth: 200, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                    {fw.description ?? '\u2014'}
                  </td>
                  <td className="actions-column">
                    {fw.has_blob && (
                      <a
                        href={fw.url}
                        download
                        style={{ marginRight: 4 }}
                      >
                        <Button
                          icon="download"
                          minimal
                          small
                          title="Download"
                        />
                      </a>
                    )}
                    <Button
                      icon="trash"
                      minimal
                      small
                      intent="danger"
                      loading={deleteMutation.isPending && deleteMutation.variables === fw.id}
                      onClick={(e) => {
                        e.stopPropagation();
                        deleteMutation.mutate(fw.id);
                      }}
                      title="Delete"
                    />
                  </td>
                </tr>
              ))}
            </tbody>
          </HTMLTable>
        )}
      </Card>

      <Dialog
        icon="upload"
        title="Register Firmware Update"
        isOpen={isAddDialogOpen}
        onClose={() => setIsAddDialogOpen(false)}
      >
        <DialogBody>
          <FormGroup label="Device Type" labelInfo="(required)">
            <HTMLSelect
              value={selectedDeviceTypeId ?? ''}
              onChange={(e) => {
                setSelectedDeviceTypeId(Number(e.target.value));
                setVersion('');
              }}
              fill
            >
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
            <FormGroup label="Firmware File" labelInfo="(required)" helperText="Binary will be stored in the database. SHA-256 is computed automatically.">
              <input
                ref={fileInputRef}
                type="file"
                style={{ display: 'none' }}
                onChange={(e) => setSelectedFile(e.target.files?.[0] ?? null)}
              />
              <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                <Button
                  icon="document-open"
                  onClick={() => fileInputRef.current?.click()}
                >
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
              <FormGroup label="SHA-256 Hash" helperText="Optional hash for binary verification on device">
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
              <Button onClick={() => setIsAddDialogOpen(false)}>Cancel</Button>
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
    </div>
  );
};
