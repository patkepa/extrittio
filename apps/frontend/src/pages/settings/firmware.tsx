import { useState } from 'react';
import { showSuccessToast, showErrorToast } from '../../utils/toaster';
import {
  Card,
  Elevation,
  H3,
  H4,
  HTMLTable,
  Button,
  Callout,
  Tag,
  Spinner,
  HTMLSelect,
  Icon,
} from '@blueprintjs/core';
import { useDeviceTypes } from '../../hooks/use-device-types';
import { useFirmwareUpdates, useDeleteFirmwareUpdate } from '../../hooks/use-firmware-updates';
import { AddFirmwareDialog } from '../../components/settings/add-firmware-dialog';
import './settings.css';

export const FirmwareSettings = () => {
  const [isAddDialogOpen, setIsAddDialogOpen] = useState(false);
  const [filterDeviceTypeId, setFilterDeviceTypeId] = useState<number | undefined>(undefined);

  const { data: deviceTypes = [] } = useDeviceTypes();
  const {
    data: firmwareUpdates = [],
    isLoading,
    error,
  } = useFirmwareUpdates(filterDeviceTypeId ? { device_type_id: filterDeviceTypeId } : undefined);
  const deleteMutation = useDeleteFirmwareUpdate();

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
            {firmwareUpdates.length} firmware release{firmwareUpdates.length !== 1 ? 's' : ''}{' '}
            registered
          </p>
        </div>
        <Button intent="primary" icon="add" onClick={() => setIsAddDialogOpen(true)}>
          Add Firmware
        </Button>
      </div>

      <div style={{ marginBottom: 16 }}>
        <HTMLSelect
          value={filterDeviceTypeId ?? ''}
          onChange={(e) =>
            setFilterDeviceTypeId(e.target.value ? Number(e.target.value) : undefined)
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

      <Card elevation={Elevation.ONE} className="settings-table-card">
        {firmwareUpdates.length === 0 ? (
          <div className="settings-empty">
            <Icon icon="updated" size={48} />
            <H4>No firmware updates</H4>
            <p>Upload firmware to enable OTA updates</p>
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
                    {fw.source === 'ci' && (
                      <div style={{ fontSize: 12, marginTop: 4, opacity: 0.8 }}>
                        {fw.commit_sha && (
                          <span style={{ marginRight: 12 }}>
                            Commit: <code>{fw.commit_sha.substring(0, 7)}</code>
                          </span>
                        )}
                        {fw.branch && <span style={{ marginRight: 12 }}>Branch: {fw.branch}</span>}
                        {fw.ci_run_url && (
                          <a href={fw.ci_run_url} target="_blank" rel="noopener noreferrer">
                            CI Run
                          </a>
                        )}
                      </div>
                    )}
                  </td>
                  <td>{fw.device_type_name}</td>
                  <td>
                    <Tag minimal intent={fw.source === 'ci' ? 'primary' : 'none'}>
                      {fw.source === 'ci' ? 'CI' : 'Manual'}
                    </Tag>
                  </td>
                  <td
                    style={{
                      maxWidth: 200,
                      overflow: 'hidden',
                      textOverflow: 'ellipsis',
                      whiteSpace: 'nowrap',
                    }}
                  >
                    {fw.description ?? '\u2014'}
                  </td>
                  <td className="actions-column">
                    {fw.has_blob && (
                      <a href={fw.url} download style={{ marginRight: 4 }}>
                        <Button icon="download" minimal small title="Download" />
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
                        deleteMutation.mutate(fw.id, {
                          onSuccess: () => void showSuccessToast('Firmware deleted'),
                          onError: () => void showErrorToast('Failed to delete firmware'),
                        });
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

      <AddFirmwareDialog isOpen={isAddDialogOpen} onClose={() => setIsAddDialogOpen(false)} />
    </div>
  );
};
