import {
  Alert,
  Button,
  Icon,
  Menu,
  MenuDivider,
  MenuItem,
  Popover,
  Tooltip,
} from '@blueprintjs/core';
import { useRef, useState, type ReactNode } from 'react';
import { BottomToolbar } from '../layout/bottom-toolbar';
import { useFleets } from '../../hooks/use-fleets';
import {
  useBulkChangeFleet,
  useBulkRestartDevices,
  useBulkTriggerOta,
} from '../../hooks/use-devices';
import { useFirmwareUpdates } from '../../hooks/use-firmware-updates';
import { useSelectionStore } from '../../stores/selection-store';
import { showErrorToast, showSuccessToast, showWarningToast } from '../../utils/toaster';
import type { Device } from '../../types/api';

interface FleetGraphBottomToolbarProps {
  deviceCount: number;
  hasGraphData: boolean;
  visibleDevices: Device[];
  displayOptions: FleetGraphDisplayOptions;
  onDisplayOptionsChange: (options: FleetGraphDisplayOptions) => void;
}

export type DisplayToggleKey = 'labels' | 'alerts' | 'fleets' | 'offline';

export type FleetGraphDisplayOptions = Record<DisplayToggleKey, boolean>;

const toolbarWideButtonClass = 'panel-toolbar-button panel-toolbar-button--wide';

interface ToolbarTooltipProps {
  title: string;
  description: string;
  children: ReactNode;
}

const ToolbarTooltip = ({ title, description, children }: ToolbarTooltipProps) => (
  <Tooltip
    content={
      <div className="panel-toolbar-tooltip">
        <span className="panel-toolbar-tooltip-title">{title}</span>
        <span className="panel-toolbar-tooltip-description">{description}</span>
      </div>
    }
    hoverOpenDelay={150}
    minimal
    modifiers={{ offset: { enabled: true, options: { offset: [0, 10] } } }}
    placement="top"
    popoverClassName="panel-toolbar-tooltip-popover"
  >
    {children}
  </Tooltip>
);

const actionConfig = [
  {
    icon: 'flows' as const,
    label: 'Assign fleet',
    description: 'Move the current device set into a fleet.',
  },
  {
    icon: 'refresh' as const,
    label: 'Restart',
    description: 'Queue a restart command for selected devices.',
  },
  {
    icon: 'cloud-upload' as const,
    label: 'Trigger OTA',
    description: 'Start a firmware rollout for this device set.',
  },
  {
    icon: 'export' as const,
    label: 'Export',
    description: 'Export the visible or selected device list.',
  },
];

const displayToggleConfig: Array<{
  key: DisplayToggleKey;
  icon: 'tag' | 'notifications' | 'flows' | 'offline';
  label: string;
  description: string;
}> = [
  {
    key: 'labels',
    icon: 'tag',
    label: 'Device labels',
    description: 'Show device names directly on the graph.',
  },
  {
    key: 'alerts',
    icon: 'notifications',
    label: 'Alert badges',
    description: 'Show active alert indicators on devices.',
  },
  {
    key: 'fleets',
    icon: 'flows',
    label: 'Fleet groups',
    description: 'Show fleet grouping hints in the topology.',
  },
  {
    key: 'offline',
    icon: 'offline',
    label: 'Offline devices',
    description: 'Include offline devices in the graph view.',
  },
];

function toRawDeviceId(id: string) {
  return id.replace(/^device-/, '');
}

function csvEscape(value: unknown) {
  const text = value == null ? '' : String(value);
  if (!/[",\n\r]/.test(text)) return text;
  return `"${text.replace(/"/g, '""')}"`;
}

export const FleetGraphBottomToolbar = ({
  deviceCount,
  hasGraphData,
  visibleDevices,
  displayOptions,
  onDisplayOptionsChange,
}: FleetGraphBottomToolbarProps) => {
  const selectedDeviceIds = useSelectionStore((state) => state.selectedDeviceIds);
  const clearSelection = useSelectionStore((state) => state.clearSelection);
  const selectedCount = selectedDeviceIds.size;
  const restartIdsRef = useRef<string[]>([]);
  const [restartCount, setRestartCount] = useState(0);
  const [restartAlertOpen, setRestartAlertOpen] = useState(false);
  const { data: fleets = [] } = useFleets();
  const { data: firmwareUpdates } = useFirmwareUpdates();
  const bulkFleetMutation = useBulkChangeFleet();
  const bulkRestartMutation = useBulkRestartDevices();
  const bulkOtaMutation = useBulkTriggerOta();
  const isAnyPending =
    bulkFleetMutation.isPending || bulkRestartMutation.isPending || bulkOtaMutation.isPending;

  const selectedRawIds = Array.from(selectedDeviceIds).map(toRawDeviceId);
  const targetDeviceIds =
    selectedRawIds.length > 0 ? selectedRawIds : visibleDevices.map((device) => device.id);
  const targetCount = targetDeviceIds.length;
  const actionTarget =
    selectedCount > 0
      ? `${selectedCount.toLocaleString()} device${selectedCount !== 1 ? 's' : ''} selected`
      : `${deviceCount.toLocaleString()} device${deviceCount !== 1 ? 's' : ''} visible`;

  const toggleDisplay = (key: DisplayToggleKey) => {
    onDisplayOptionsChange({ ...displayOptions, [key]: !displayOptions[key] });
  };

  async function handleFleetChange(fleetId: number | null) {
    try {
      const result = await bulkFleetMutation.mutateAsync({
        device_ids: targetDeviceIds,
        fleet_id: fleetId,
      });
      void showSuccessToast(
        fleetId === null
          ? `${result.affected} device${result.affected !== 1 ? 's' : ''} removed from fleet`
          : `${result.affected} device${result.affected !== 1 ? 's' : ''} moved to fleet`,
      );
      clearSelection();
    } catch {
      void showErrorToast('Failed to change fleet');
    }
  }

  async function handleRestart() {
    try {
      const result = await bulkRestartMutation.mutateAsync({ device_ids: restartIdsRef.current });
      if (result.failed > 0) {
        void showWarningToast(`${result.succeeded} restarted, ${result.failed} failed`);
        useSelectionStore.setState({
          selectedDeviceIds: new Set(result.errors.map((e) => `device-${e.device_id}`)),
          isAllMatchingSelected: false,
          selectionFilters: null,
        });
      } else {
        void showSuccessToast(
          `${result.succeeded} device${result.succeeded !== 1 ? 's' : ''} restarted`,
        );
        clearSelection();
      }
    } catch {
      void showErrorToast('Failed to restart devices');
    }
    setRestartAlertOpen(false);
  }

  async function handleOta(firmwareUpdateId: number) {
    try {
      const result = await bulkOtaMutation.mutateAsync({
        device_ids: targetDeviceIds,
        firmware_update_id: firmwareUpdateId,
      });
      if (result.failed > 0) {
        void showWarningToast(`${result.succeeded} updated, ${result.failed} failed`);
        useSelectionStore.setState({
          selectedDeviceIds: new Set(result.errors.map((e) => `device-${e.device_id}`)),
          isAllMatchingSelected: false,
          selectionFilters: null,
        });
      } else {
        void showSuccessToast(
          `OTA triggered on ${result.succeeded} device${result.succeeded !== 1 ? 's' : ''}`,
        );
        clearSelection();
      }
    } catch {
      void showErrorToast('Failed to trigger OTA update');
    }
  }

  function handleExport() {
    const selectedSet = new Set(selectedRawIds);
    const rows =
      selectedSet.size > 0
        ? visibleDevices.filter((device) => selectedSet.has(device.id))
        : visibleDevices;

    const headers = [
      'id',
      'name',
      'status',
      'device_type',
      'fleet',
      'firmware',
      'last_seen',
      'uptime',
    ];
    const lines = [
      headers.join(','),
      ...rows.map((device) =>
        [
          device.id,
          device.name,
          device.status,
          device.device_type_name,
          device.fleet_name ?? '',
          device.firmware,
          device.last_seen,
          device.uptime,
        ]
          .map(csvEscape)
          .join(','),
      ),
    ];

    const blob = new Blob([`${lines.join('\n')}\n`], { type: 'text/csv;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = `fleet-graph-devices-${new Date().toISOString().slice(0, 10)}.csv`;
    anchor.click();
    URL.revokeObjectURL(url);
    void showSuccessToast(`Exported ${rows.length} device${rows.length !== 1 ? 's' : ''}`);
  }

  const actionDisabled = !hasGraphData || targetCount === 0 || isAnyPending;

  const renderActionButton = (label: string) => {
    switch (label) {
      case 'Assign fleet':
        return (
          <Popover
            content={
              <Menu>
                {fleets.map((fleet) => (
                  <MenuItem
                    key={fleet.id}
                    text={fleet.name}
                    onClick={() => void handleFleetChange(fleet.id)}
                  />
                ))}
                {fleets.length > 0 && <MenuDivider />}
                <MenuItem
                  text="Remove from fleet"
                  icon="cross"
                  intent="warning"
                  onClick={() => void handleFleetChange(null)}
                />
              </Menu>
            }
            placement="top"
            disabled={actionDisabled}
          >
            <Button
              className={toolbarWideButtonClass}
              icon="flows"
              text="Assign fleet"
              small
              loading={bulkFleetMutation.isPending}
              disabled={actionDisabled}
              aria-label="Assign fleet"
            />
          </Popover>
        );
      case 'Restart':
        return (
          <Button
            className={toolbarWideButtonClass}
            icon="refresh"
            text="Restart"
            small
            loading={bulkRestartMutation.isPending}
            disabled={actionDisabled}
            aria-label="Restart"
            onClick={() => {
              restartIdsRef.current = targetDeviceIds;
              setRestartCount(targetDeviceIds.length);
              setRestartAlertOpen(true);
            }}
          />
        );
      case 'Trigger OTA':
        return (
          <Popover
            content={
              <Menu>
                {(firmwareUpdates ?? []).map((fw) => (
                  <MenuItem
                    key={fw.id}
                    text={`${fw.version} (${fw.device_type_name})`}
                    onClick={() => void handleOta(fw.id)}
                  />
                ))}
                {(firmwareUpdates ?? []).length === 0 && (
                  <MenuItem text="No firmware updates available" disabled />
                )}
              </Menu>
            }
            placement="top"
            disabled={actionDisabled}
          >
            <Button
              className={toolbarWideButtonClass}
              icon="cloud-upload"
              text="Trigger OTA"
              small
              loading={bulkOtaMutation.isPending}
              disabled={actionDisabled}
              aria-label="Trigger OTA"
            />
          </Popover>
        );
      case 'Export':
        return (
          <Button
            className={toolbarWideButtonClass}
            icon="export"
            text="Export"
            small
            disabled={!hasGraphData || targetCount === 0}
            aria-label="Export"
            onClick={handleExport}
          />
        );
      default:
        return null;
    }
  };

  return (
    <>
      <BottomToolbar
        className="fleet-graph-toolbar-shell"
        ariaLabel="Fleet graph multi-device toolbar"
      >
        <div className="fleet-graph-toolbar fleet-graph-toolbar--bottom">
          <div className="fleet-graph-toolbar-identity">
            <Icon icon="multi-select" size={16} />
            <div className="fleet-graph-toolbar-title-group">
              <span className="fleet-graph-toolbar-title">Multi-device actions</span>
              <span className="fleet-graph-toolbar-subtitle">{actionTarget}</span>
            </div>
          </div>

          <div className="fleet-graph-toolbar-divider" aria-hidden="true" />

          <div className="fleet-graph-toolbar-actions" aria-label="Display toggles">
            {displayToggleConfig.map((toggle) => {
              const isActive = displayOptions[toggle.key];
              const verb = isActive ? 'Hide' : 'Show';
              return (
                <ToolbarTooltip
                  key={toggle.key}
                  title={`${verb} ${toggle.label.toLowerCase()}`}
                  description={toggle.description}
                >
                  <Button
                    className={`panel-toolbar-switch${isActive ? '' : ' panel-toolbar-switch--off'}`}
                    icon={toggle.icon}
                    minimal
                    small
                    disabled={!hasGraphData}
                    aria-label={`${verb} ${toggle.label.toLowerCase()}`}
                    aria-pressed={isActive}
                    onClick={() => toggleDisplay(toggle.key)}
                  />
                </ToolbarTooltip>
              );
            })}
          </div>

          <div className="fleet-graph-toolbar-divider" aria-hidden="true" />

          <div className="fleet-graph-toolbar-actions">
            {actionConfig.map((action) => (
              <ToolbarTooltip
                key={action.label}
                title={action.label}
                description={action.description}
              >
                {renderActionButton(action.label)}
              </ToolbarTooltip>
            ))}
          </div>
          <div className="fleet-graph-toolbar-spacer" />
        </div>
      </BottomToolbar>

      <Alert
        isOpen={restartAlertOpen}
        icon="refresh"
        intent="warning"
        confirmButtonText="Restart"
        cancelButtonText="Cancel"
        onConfirm={() => void handleRestart()}
        onCancel={() => setRestartAlertOpen(false)}
      >
        <p>
          Restart <strong>{restartCount}</strong> device{restartCount !== 1 ? 's' : ''}?
        </p>
      </Alert>
    </>
  );
};
