import { useEffect, useState } from 'react';
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
import { CertificateDownloadDialog } from '../certificates/certificate-download-dialog';
import { getDeviceCertificate } from '../../api/certificates';
import { useCaCertificate } from '../../hooks/use-certificates';
import type { DeviceCertificateResponse } from '../../types/api';

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
  });

  const [certBundle, setCertBundle] = useState<DeviceCertificateResponse | null>(null);
  const [createdDeviceName, setCreatedDeviceName] = useState('');
  const caQuery = useCaCertificate();

  // Reset stale mutation error when dialog opens/closes
  useEffect(() => {
    if (!isAddDeviceDialogOpen) {
      createDeviceMutation.reset();
    }
  }, [isAddDeviceDialogOpen]);

  const handleAddDevice = () => {
    createDeviceMutation.mutate(
      {
        name: newDevice.name,
        device_type_id: newDevice.device_type_id || defaultTypeId,
        fleet_id: newDevice.fleet_id,
      },
      {
        onSuccess: async (device) => {
          void showSuccessToast('Device added');
          // If CA exists, fetch certificate bundle (one-shot, imperative)
          if (caQuery.data) {
            try {
              const bundle = await getDeviceCertificate(device.id);
              setCreatedDeviceName(device.name);
              setCertBundle(bundle);
              return; // Don't close dialog — show certificate step
            } catch {
              // Certificate fetch failed — close normally
            }
          }
          closeAddDeviceDialog();
          setNewDevice({ name: '', device_type_id: 0, fleet_id: undefined });
        },
        onError: () => {
          void showErrorToast('Failed to add device');
        },
      }
    );
  };

  return (
    <>
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
                disabled={!newDevice.name.trim() || deviceTypes.length === 0}
              >
                Add Device
              </Button>
            </>
          }
        />
      </Dialog>
      <CertificateDownloadDialog
        isOpen={certBundle !== null}
        onClose={() => {
          setCertBundle(null);
          setCreatedDeviceName('');
          closeAddDeviceDialog();
          setNewDevice({ name: '', device_type_id: 0, fleet_id: undefined });
        }}
        certBundle={certBundle}
        deviceName={createdDeviceName}
      />
    </>
  );
}
