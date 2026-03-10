import { useState } from 'react';
import {
  Button,
  Callout,
  Dialog,
  DialogBody,
  DialogFooter,
  FormGroup,
  HTMLSelect,
  InputGroup,
} from '@blueprintjs/core';
import { useCreateDevice } from '../../hooks/use-devices';
import { useDeviceTypes } from '../../hooks/use-device-types';
import { useFleets } from '../../hooks/use-fleets';
import { useUIStore } from '../../stores/ui-store';
import { showSuccessToast, showErrorToast } from '../../utils/toaster';

export function AddDeviceDialog() {
  const { isAddDeviceDialogOpen, closeAddDeviceDialog } = useUIStore();
  const { data: deviceTypes = [] } = useDeviceTypes();
  const { data: fleets = [] } = useFleets();
  const createDeviceMutation = useCreateDevice();

  const defaultTypeId = deviceTypes.find((dt) => dt.name === 'default')?.id ?? deviceTypes[0]?.id ?? 0;

  const [newDevice, setNewDevice] = useState({
    name: '',
    device_type_id: 0,
    fleet_id: undefined as number | undefined,
    location: '',
    firmware: '',
  });

  const handleAddDevice = () => {
    createDeviceMutation.mutate(
      {
        name: newDevice.name,
        device_type_id: newDevice.device_type_id || defaultTypeId,
        fleet_id: newDevice.fleet_id,
        location: newDevice.location || undefined,
        firmware: newDevice.firmware || undefined,
      },
      {
        onSuccess: () => {
          closeAddDeviceDialog();
          setNewDevice({ name: '', device_type_id: 0, fleet_id: undefined, location: '', firmware: '' });
          void showSuccessToast('Device added');
        },
        onError: () => {
          void showErrorToast('Failed to add device');
        },
      }
    );
  };

  return (
    <Dialog icon="add" title="Add Device" isOpen={isAddDeviceDialogOpen} onClose={closeAddDeviceDialog}>
      <DialogBody>
        <FormGroup label="Name" labelInfo="(required)">
          <InputGroup
            placeholder="e.g. Temperature Sensor A1"
            value={newDevice.name}
            onChange={(e) => setNewDevice({ ...newDevice, name: e.target.value })}
          />
        </FormGroup>
        <FormGroup label="Device Type" labelInfo="(required)">
          <HTMLSelect
            fill
            value={newDevice.device_type_id || defaultTypeId}
            onChange={(e) => setNewDevice({ ...newDevice, device_type_id: Number(e.target.value) })}
          >
            {deviceTypes.map((dt) => (
              <option key={dt.id} value={dt.id}>{dt.name}</option>
            ))}
          </HTMLSelect>
        </FormGroup>
        <FormGroup label="Fleet">
          <HTMLSelect
            fill
            value={newDevice.fleet_id ?? ''}
            onChange={(e) =>
              setNewDevice({ ...newDevice, fleet_id: e.target.value ? Number(e.target.value) : undefined })
            }
          >
            <option value="">No fleet</option>
            {fleets.map((f) => (
              <option key={f.id} value={f.id}>{f.name}</option>
            ))}
          </HTMLSelect>
        </FormGroup>
        <FormGroup label="Location">
          <InputGroup
            placeholder="e.g. Building A, Floor 2"
            value={newDevice.location}
            onChange={(e) => setNewDevice({ ...newDevice, location: e.target.value })}
          />
        </FormGroup>
        <FormGroup label="Firmware">
          <InputGroup
            placeholder="e.g. v1.2.0"
            value={newDevice.firmware}
            onChange={(e) => setNewDevice({ ...newDevice, firmware: e.target.value })}
          />
        </FormGroup>
        {createDeviceMutation.isError && (
          <Callout intent="danger" icon="error">Failed to create device. Please try again.</Callout>
        )}
      </DialogBody>
      <DialogFooter
        actions={
          <>
            <Button onClick={closeAddDeviceDialog}>Cancel</Button>
            <Button
              intent="primary"
              icon="add"
              onClick={handleAddDevice}
              loading={createDeviceMutation.isPending}
              disabled={!newDevice.name.trim()}
            >
              Add Device
            </Button>
          </>
        }
      />
    </Dialog>
  );
}
