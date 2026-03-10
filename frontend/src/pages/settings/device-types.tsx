import { useState } from 'react';
import { showSuccessToast, showErrorToast } from '../../utils/toaster';
import {
  Card,
  Elevation,
  H3,
  H4,
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
  Icon,
} from '@blueprintjs/core';
import {
  useDeviceTypes,
  useCreateDeviceType,
  useDeleteDeviceType,
} from '../../hooks/use-device-types';
import './settings.css';

export const DeviceTypesSettings = () => {
  const [isAddDialogOpen, setIsAddDialogOpen] = useState(false);
  const [newName, setNewName] = useState('');

  const { data: deviceTypes = [], isLoading, error } = useDeviceTypes();
  const createMutation = useCreateDeviceType();
  const deleteMutation = useDeleteDeviceType();

  const handleAdd = () => {
    createMutation.mutate(
      { name: newName.trim() },
      {
        onSuccess: () => {
          setIsAddDialogOpen(false);
          setNewName('');
          void showSuccessToast('Device type created');
        },
        onError: () => { void showErrorToast('Failed to create device type'); },
      }
    );
  };

  if (error) {
    return (
      <div className="settings-page">
        <Callout intent="danger" icon="error">
          Failed to load device types. Is the backend running?
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
          <H3>Device Types</H3>
          <p className="page-description">
            {deviceTypes.length} device type{deviceTypes.length !== 1 ? 's' : ''}
          </p>
        </div>
        <Button intent="primary" icon="add" onClick={() => setIsAddDialogOpen(true)}>
          Add Device Type
        </Button>
      </div>

      <Card elevation={Elevation.TWO} className="settings-table-card">
        {deviceTypes.length === 0 ? (
          <div className="settings-empty">
            <Icon icon="cube" size={48} />
            <H4>No device types</H4>
            <p>Add a device type to categorize your devices</p>
          </div>
        ) : (
          <HTMLTable interactive className="settings-table">
            <thead>
              <tr>
                <th>Name</th>
                <th className="actions-column">Actions</th>
              </tr>
            </thead>
            <tbody>
              {deviceTypes.map((dt) => (
                <tr key={dt.id}>
                  <td>
                    <span style={{ fontWeight: 600 }}>{dt.name}</span>
                    {dt.id === 1 && (
                      <Tag minimal intent="primary" style={{ marginLeft: 8 }}>
                        default
                      </Tag>
                    )}
                  </td>
                  <td className="actions-column">
                    <Button
                      icon="trash"
                      minimal
                      small
                      intent="danger"
                      disabled={dt.id === 1}
                      loading={deleteMutation.isPending && deleteMutation.variables === dt.id}
                      onClick={() =>
                        deleteMutation.mutate(dt.id, {
                          onSuccess: () => void showSuccessToast('Device type deleted'),
                          onError: () => void showErrorToast('Failed to delete device type'),
                        })
                      }
                      title={dt.id === 1 ? 'Cannot delete default type' : 'Delete'}
                    />
                  </td>
                </tr>
              ))}
            </tbody>
          </HTMLTable>
        )}
      </Card>

      {deleteMutation.isError && (
        <Callout intent="warning" icon="warning-sign" style={{ marginTop: 12 }}>
          Cannot delete this device type — devices are still assigned to it. Reassign them first.
        </Callout>
      )}

      <Dialog
        icon="add"
        title="Add Device Type"
        isOpen={isAddDialogOpen}
        onClose={() => setIsAddDialogOpen(false)}
      >
        <DialogBody>
          <FormGroup label="Name" labelInfo="(required)">
            <InputGroup
              placeholder="e.g. sensor, gateway, actuator"
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              autoFocus
            />
          </FormGroup>
          {createMutation.isError && (
            <Callout intent="danger" icon="error">
              Failed to create device type. The name may already exist.
            </Callout>
          )}
        </DialogBody>
        <DialogFooter
          actions={
            <>
              <Button onClick={() => setIsAddDialogOpen(false)}>Cancel</Button>
              <Button
                intent="primary"
                icon="add"
                onClick={handleAdd}
                loading={createMutation.isPending}
                disabled={!newName.trim()}
              >
                Add
              </Button>
            </>
          }
        />
      </Dialog>
    </div>
  );
};
