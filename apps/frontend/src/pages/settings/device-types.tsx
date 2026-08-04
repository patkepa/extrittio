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
  ButtonGroup,
} from '@blueprintjs/core';
import type { IconName } from '@blueprintjs/core';
import {
  useDeviceTypes,
  useCreateDeviceType,
  useUpdateDeviceType,
  useDeleteDeviceType,
} from '../../hooks/use-device-types';
import { useConfirmShortcut } from '@extrittio/interactions';
import type { DeviceType } from '../../types/api';
import './settings.css';

const DEFAULT_ICON = 'cube';
const DEFAULT_COLOR_HEX = '#8ABBFF';

const DEVICE_TYPE_ICONS: IconName[] = [
  'cube',
  'sensor',
  'desktop',
  'mobile-phone',
  'antenna',
  'cell-tower',
  'satellite',
  'dashboard',
  'pulse',
  'heatmap',
  'cloud',
  'database',
  'wrench',
  'cog',
  'truck',
  'map-marker',
];

const DEVICE_TYPE_COLORS = [
  '#8ABBFF',
  '#36CFC9',
  '#7BD88F',
  '#F7C948',
  '#F29D49',
  '#E76A6E',
  '#D982FF',
  '#A7B0C0',
];

function isValidColorHex(value: string) {
  return /^#[0-9A-Fa-f]{6}$/.test(value);
}

function normalizeColorHex(value: string) {
  return value.trim().toUpperCase();
}

interface VisualPickerProps {
  icon: string;
  colorHex: string;
  onIconChange: (icon: string) => void;
  onColorHexChange: (colorHex: string) => void;
}

const DeviceTypeVisualPicker = ({
  icon,
  colorHex,
  onIconChange,
  onColorHexChange,
}: VisualPickerProps) => (
  <>
    <FormGroup label="Icon">
      <ButtonGroup className="device-type-icon-grid">
        {DEVICE_TYPE_ICONS.map((option) => (
          <Button
            key={option}
            icon={option}
            small
            active={icon === option}
            onClick={() => onIconChange(option)}
            title={option}
            aria-label={option}
          />
        ))}
      </ButtonGroup>
    </FormGroup>

    <FormGroup label="Color">
      <div className="device-type-color-grid">
        {DEVICE_TYPE_COLORS.map((option) => (
          <button
            key={option}
            type="button"
            className={`device-type-color-swatch ${normalizeColorHex(colorHex) === option ? 'active' : ''}`}
            style={{ backgroundColor: option }}
            onClick={() => onColorHexChange(option)}
            aria-label={option}
            title={option}
          />
        ))}
      </div>
      <div className="device-type-color-inputs">
        <input
          type="color"
          value={isValidColorHex(colorHex) ? colorHex : DEFAULT_COLOR_HEX}
          onChange={(e) => onColorHexChange(normalizeColorHex(e.target.value))}
          aria-label="Color picker"
        />
        <InputGroup
          value={colorHex}
          onChange={(e) => onColorHexChange(e.target.value)}
          placeholder="#8ABBFF"
          intent={isValidColorHex(colorHex) ? undefined : 'danger'}
        />
      </div>
    </FormGroup>
  </>
);

export const DeviceTypesSettings = () => {
  const [isAddDialogOpen, setIsAddDialogOpen] = useState(false);
  const [newName, setNewName] = useState('');
  const [newIcon, setNewIcon] = useState(DEFAULT_ICON);
  const [newColorHex, setNewColorHex] = useState(DEFAULT_COLOR_HEX);
  const [editingDeviceType, setEditingDeviceType] = useState<DeviceType | null>(null);
  const [editName, setEditName] = useState('');
  const [editIcon, setEditIcon] = useState(DEFAULT_ICON);
  const [editColorHex, setEditColorHex] = useState(DEFAULT_COLOR_HEX);

  const { data: deviceTypes = [], isLoading, error } = useDeviceTypes();
  const createMutation = useCreateDeviceType();
  const updateMutation = useUpdateDeviceType();
  const deleteMutation = useDeleteDeviceType();

  const resetAddDialog = () => {
    setNewName('');
    setNewIcon(DEFAULT_ICON);
    setNewColorHex(DEFAULT_COLOR_HEX);
  };

  const openEditDialog = (deviceType: DeviceType) => {
    setEditingDeviceType(deviceType);
    setEditName(deviceType.name);
    setEditIcon(deviceType.icon || DEFAULT_ICON);
    setEditColorHex(deviceType.color_hex || DEFAULT_COLOR_HEX);
  };

  const handleAdd = () => {
    createMutation.mutate(
      {
        name: newName.trim(),
        icon: newIcon,
        color_hex: normalizeColorHex(newColorHex),
      },
      {
        onSuccess: () => {
          setIsAddDialogOpen(false);
          resetAddDialog();
          void showSuccessToast('Device type created');
        },
        onError: () => {
          void showErrorToast('Failed to create device type');
        },
      },
    );
  };

  const handleUpdate = () => {
    if (!editingDeviceType) return;
    updateMutation.mutate(
      {
        id: editingDeviceType.id,
        body: {
          name: editName.trim(),
          icon: editIcon,
          color_hex: normalizeColorHex(editColorHex),
        },
      },
      {
        onSuccess: () => {
          setEditingDeviceType(null);
          void showSuccessToast('Device type updated');
        },
        onError: () => {
          void showErrorToast('Failed to update device type');
        },
      },
    );
  };

  const canAddDeviceType =
    !!newName.trim() && isValidColorHex(newColorHex) && !createMutation.isPending;
  const canUpdateDeviceType =
    !!editName.trim() && isValidColorHex(editColorHex) && !updateMutation.isPending;

  useConfirmShortcut({
    isOpen: isAddDialogOpen,
    canConfirm: canAddDeviceType,
    onConfirm: handleAdd,
  });

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

      <Card elevation={Elevation.ONE} className="settings-table-card">
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
                    <span className="device-type-name-cell">
                      <Icon icon={(dt.icon || DEFAULT_ICON) as IconName} color={dt.color_hex} />
                      <span style={{ fontWeight: 600 }}>{dt.name}</span>
                      {dt.id === 1 && (
                        <Tag minimal intent="primary">
                          default
                        </Tag>
                      )}
                    </span>
                  </td>
                  <td className="actions-column">
                    <Button
                      icon="edit"
                      minimal
                      small
                      onClick={() => openEditDialog(dt)}
                      title="Edit"
                    />
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
          <DeviceTypeVisualPicker
            icon={newIcon}
            colorHex={newColorHex}
            onIconChange={setNewIcon}
            onColorHexChange={setNewColorHex}
          />
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
                disabled={!canAddDeviceType}
              >
                Add
              </Button>
            </>
          }
        />
      </Dialog>

      <Dialog
        icon="edit"
        title="Edit Device Type"
        isOpen={editingDeviceType !== null}
        onClose={() => setEditingDeviceType(null)}
      >
        <DialogBody>
          <FormGroup label="Name" labelInfo="(required)">
            <InputGroup
              placeholder="e.g. sensor, gateway, actuator"
              value={editName}
              onChange={(e) => setEditName(e.target.value)}
              disabled={editingDeviceType?.id === 1}
              autoFocus
            />
          </FormGroup>
          <DeviceTypeVisualPicker
            icon={editIcon}
            colorHex={editColorHex}
            onIconChange={setEditIcon}
            onColorHexChange={setEditColorHex}
          />
          {updateMutation.isError && (
            <Callout intent="danger" icon="error">
              Failed to update device type. The name may already exist.
            </Callout>
          )}
        </DialogBody>
        <DialogFooter
          actions={
            <>
              <Button onClick={() => setEditingDeviceType(null)}>Cancel</Button>
              <Button
                intent="primary"
                icon="tick"
                onClick={handleUpdate}
                loading={updateMutation.isPending}
                disabled={!canUpdateDeviceType}
              >
                Save
              </Button>
            </>
          }
        />
      </Dialog>
    </div>
  );
};
