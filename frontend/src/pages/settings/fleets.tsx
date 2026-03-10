import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { showSuccessToast, showErrorToast } from '../../utils/toaster';
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
} from '@blueprintjs/core';
import {
  useFleets,
  useCreateFleet,
  useDeleteFleet,
} from '../../hooks/use-fleets';
import './settings.css';

export const FleetsSettings = () => {
  const navigate = useNavigate();
  const [isAddDialogOpen, setIsAddDialogOpen] = useState(false);
  const [newName, setNewName] = useState('');

  const { data: fleets = [], isLoading, error } = useFleets();
  const createMutation = useCreateFleet();
  const deleteMutation = useDeleteFleet();

  const handleAdd = () => {
    createMutation.mutate(
      { name: newName.trim() },
      {
        onSuccess: () => {
          setIsAddDialogOpen(false);
          setNewName('');
          void showSuccessToast('Fleet created');
        },
        onError: () => { void showErrorToast('Failed to create fleet'); },
      }
    );
  };

  if (error) {
    return (
      <div className="settings-page">
        <Callout intent="danger" icon="error">
          Failed to load fleets. Is the backend running?
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
          <H3>Fleets</H3>
          <p className="page-description">
            {fleets.length} fleet{fleets.length !== 1 ? 's' : ''}
          </p>
        </div>
        <Button intent="primary" icon="add" onClick={() => setIsAddDialogOpen(true)}>
          Add Fleet
        </Button>
      </div>

      <Card elevation={Elevation.TWO} className="settings-table-card">
        {fleets.length === 0 ? (
          <div className="settings-empty">
            <p>No fleets yet. Create one to start grouping devices.</p>
          </div>
        ) : (
          <HTMLTable interactive className="settings-table">
            <thead>
              <tr>
                <th>Name</th>
                <th>Devices</th>
                <th className="actions-column">Actions</th>
              </tr>
            </thead>
            <tbody>
              {fleets.map((fleet) => (
                <tr
                  key={fleet.id}
                  className="fleet-row"
                  onClick={() => navigate(`/devices?fleet_id=${fleet.id}`)}
                  style={{ cursor: 'pointer' }}
                >
                  <td>
                    <span style={{ fontWeight: 600 }}>{fleet.name}</span>
                  </td>
                  <td>
                    <Tag minimal round className="mono-data">
                      {fleet.device_count}
                    </Tag>
                  </td>
                  <td className="actions-column" onClick={(e) => e.stopPropagation()}>
                    <Button
                      icon="eye-open"
                      minimal
                      small
                      title="View Devices"
                      onClick={() => navigate(`/devices?fleet_id=${fleet.id}`)}
                    />
                    <Button
                      icon="trash"
                      minimal
                      small
                      intent="danger"
                      title="Delete Fleet"
                      loading={deleteMutation.isPending && deleteMutation.variables === fleet.id}
                      onClick={() =>
                        deleteMutation.mutate(fleet.id, {
                          onSuccess: () => void showSuccessToast('Fleet deleted'),
                          onError: () => void showErrorToast('Failed to delete fleet'),
                        })
                      }
                    />
                  </td>
                </tr>
              ))}
            </tbody>
          </HTMLTable>
        )}
      </Card>

      <Dialog
        icon="add"
        title="Add Fleet"
        isOpen={isAddDialogOpen}
        onClose={() => setIsAddDialogOpen(false)}
      >
        <DialogBody>
          <FormGroup label="Name" labelInfo="(required)">
            <InputGroup
              placeholder="e.g. Factory Floor, Warehouse A"
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              autoFocus
            />
          </FormGroup>
          {createMutation.isError && (
            <Callout intent="danger" icon="error">
              Failed to create fleet. The name may already exist.
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
