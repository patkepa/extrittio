import { useEffect, useRef, useState } from 'react';
import {
  Button,
  Callout,
  Dialog,
  DialogBody,
  DialogFooter,
  FormGroup,
  HTMLSelect,
  InputGroup,
  TextArea,
} from '@blueprintjs/core';
import type { AxiosError } from 'axios';
import { useCreateDevice } from '../../hooks/use-devices';
import { getDeviceContract } from '../../api/devices';
import { useConfirmShortcut } from '@patkepa/kantzen-ui/interactions';
import { useFormNavigation } from '@patkepa/kantzen-ui/interactions';
import {
  useDeviceBlueprints,
  useLatestDeviceBlueprintRevision,
} from '../../hooks/use-device-blueprints';
import { useFleets } from '../../hooks/use-fleets';
import { useUIStore } from '../../stores/ui-store';
import { showSuccessToast, showErrorToast } from '../../utils/toaster';
import { CertificateDownloadDialog } from '../certificates/certificate-download-dialog';
import { getDeviceCertificate } from '../../api/certificates';
import { useCaCertificate } from '../../hooks/use-certificates';
import type { DeviceCertificateResponse, DeviceContract } from '../../types/api';

function getApiErrorMessage(error: unknown, fallback: string): string {
  const axiosError = error as AxiosError<{ error?: string }>;
  return axiosError?.response?.data?.error ?? fallback;
}

export function AddDeviceDialog() {
  const formRef = useRef<HTMLDivElement | null>(null);
  const { isAddDeviceDialogOpen, closeAddDeviceDialog } = useUIStore();
  const { data: blueprints = [] } = useDeviceBlueprints();
  const { data: fleets = [] } = useFleets();
  const createDeviceMutation = useCreateDevice();

  const [newDevice, setNewDevice] = useState({
    name: '',
    fleet_id: undefined as number | undefined,
  });
  const [blueprintId, setBlueprintId] = useState('');
  const [configurationText, setConfigurationText] = useState('');
  const [formError, setFormError] = useState<string | null>(null);
  const publishedBlueprints = blueprints.filter((blueprint) => blueprint.latest_revision != null);
  const defaultBlueprintId = publishedBlueprints[0]?.id;
  const selectedBlueprintId = blueprintId || defaultBlueprintId || '';
  const revisionQuery = useLatestDeviceBlueprintRevision(selectedBlueprintId);
  const blueprintDocument = revisionQuery.data?.document as unknown as {
    spec?: {
      configuration?: unknown;
    };
  };

  const [certBundle, setCertBundle] = useState<DeviceCertificateResponse | null>(null);
  const [provisionedContract, setProvisionedContract] = useState<DeviceContract | null>(null);
  const [createdDeviceName, setCreatedDeviceName] = useState('');
  const caQuery = useCaCertificate();
  const canAddDevice =
    !!newDevice.name.trim() &&
    selectedBlueprintId.length > 0 &&
    revisionQuery.data != null &&
    !createDeviceMutation.isPending &&
    certBundle === null &&
    provisionedContract === null;

  // Reset stale mutation error when dialog opens/closes
  const resetCreateMutation = createDeviceMutation.reset;
  useEffect(() => {
    if (!isAddDeviceDialogOpen) {
      resetCreateMutation();
    }
  }, [isAddDeviceDialogOpen, resetCreateMutation]);

  const handleAddDevice = () => {
    if (!revisionQuery.data) return;
    let configuration: unknown;
    try {
      configuration = configurationText.trim() ? JSON.parse(configurationText) : undefined;
      setFormError(null);
    } catch {
      setFormError('Configuration override must be valid JSON.');
      return;
    }
    createDeviceMutation.mutate(
      {
        name: newDevice.name,
        fleet_id: newDevice.fleet_id,
        blueprint_revision_id: revisionQuery.data.id,
        configuration,
      },
      {
        onSuccess: async (device) => {
          void showSuccessToast('Device added');
          try {
            const [contract, bundle] = await Promise.all([
              getDeviceContract(device.id),
              caQuery.data ? getDeviceCertificate(device.id) : Promise.resolve(null),
            ]);
            setCreatedDeviceName(device.name);
            setProvisionedContract(contract);
            setCertBundle(bundle);
            return;
          } catch {
            // Provisioning retrieval failed — the device remains available in inventory.
          }
          closeAddDeviceDialog();
          setNewDevice({ name: '', fleet_id: undefined });
          setConfigurationText('');
        },
        onError: (error) => {
          void showErrorToast(getApiErrorMessage(error, 'Failed to add device'));
        },
      },
    );
  };

  useConfirmShortcut({
    isOpen: isAddDeviceDialogOpen,
    canConfirm: canAddDevice,
    onConfirm: handleAddDevice,
  });
  useFormNavigation(formRef, isAddDeviceDialogOpen);

  return (
    <>
      <Dialog
        icon="add"
        title="Add Device"
        isOpen={isAddDeviceDialogOpen}
        onClose={closeAddDeviceDialog}
      >
        <DialogBody>
          <div ref={formRef}>
            <FormGroup label="Name" labelInfo="(required)">
              <InputGroup
                placeholder="e.g. Temperature Sensor A1"
                value={newDevice.name}
                onChange={(e) => setNewDevice({ ...newDevice, name: e.target.value })}
              />
            </FormGroup>
            <FormGroup label="Device Blueprint" labelInfo="(required)">
              <HTMLSelect
                fill
                value={selectedBlueprintId}
                onChange={(event) => {
                  setBlueprintId(event.target.value);
                  setConfigurationText('');
                  setFormError(null);
                }}
              >
                {publishedBlueprints.length === 0 && (
                  <option value="">Publish a blueprint before creating a device</option>
                )}
                {publishedBlueprints.map((blueprint) => (
                  <option key={blueprint.id} value={blueprint.id}>
                    {blueprint.name} · revision {blueprint.latest_revision}
                  </option>
                ))}
              </HTMLSelect>
            </FormGroup>
            {blueprintDocument?.spec?.configuration != null && (
              <FormGroup label="Configuration override" labelInfo="(optional JSON)">
                <TextArea
                  fill
                  rows={4}
                  placeholder='{"sampleSeconds": 30}'
                  value={configurationText}
                  onChange={(event) => setConfigurationText(event.target.value)}
                />
              </FormGroup>
            )}
            <FormGroup label="Fleet">
              <HTMLSelect
                fill
                value={newDevice.fleet_id ?? ''}
                onChange={(e) =>
                  setNewDevice({
                    ...newDevice,
                    fleet_id: e.target.value ? Number(e.target.value) : undefined,
                  })
                }
              >
                <option value="">No fleet</option>
                {fleets.map((f) => (
                  <option key={f.id} value={f.id}>
                    {f.name}
                  </option>
                ))}
              </HTMLSelect>
            </FormGroup>
            {createDeviceMutation.isError && (
              <Callout intent="danger" icon="error">
                {getApiErrorMessage(
                  createDeviceMutation.error,
                  'Failed to create device. Please try again.',
                )}
              </Callout>
            )}
            {formError && (
              <Callout intent="danger" icon="error">
                {formError}
              </Callout>
            )}
          </div>
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
                disabled={!canAddDevice}
              >
                Add Device
              </Button>
            </>
          }
        />
      </Dialog>
      <CertificateDownloadDialog
        isOpen={certBundle !== null || provisionedContract !== null}
        onClose={() => {
          setCertBundle(null);
          setProvisionedContract(null);
          setCreatedDeviceName('');
          closeAddDeviceDialog();
          setNewDevice({ name: '', fleet_id: undefined });
          setConfigurationText('');
        }}
        certBundle={certBundle}
        contract={provisionedContract}
        deviceName={createdDeviceName}
      />
    </>
  );
}
